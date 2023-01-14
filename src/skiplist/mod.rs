extern crate alloc;
extern crate rand;

mod node;
mod padded;
mod tagged;

use std::{
    fmt::Debug,
    ptr::NonNull,
    sync::atomic::{fence, AtomicUsize, Ordering},
};

use self::{
    node::{Head, Node},
    padded::Padded,
};

const HEIGHT_BITS: usize = 5;
const HEIGHT: usize = 1 << HEIGHT_BITS;
const HEIGHT_MASK: usize = (1 << (HEIGHT_BITS + 1)) - 1;

/// A lock-free skip list. Implemented using dynamically-allocated,
/// multi-leveled `Node` towers.
///
/// # Design
/// The SkipList consists of a sorted, multi-leveled linked list, where not
/// every [Node](node::Node) is present at each level. On average, there are
/// twice as many [Nodes](node::Node) on level `i` than on level `i+1`, which is
/// our list **invariant**. This allows us to traverse the list in search for a
/// value and get the minimum value with average time complexities of
/// `O(log(n))` and `O(1)` respectively.
///
/// We achieve this invariant of `|level[i]|
/// == 2 * |level[i+1]|` by randomizing the height of our [Nodes](node::Node) on
/// insertion.
///
/// We get our claimed lookup speeds by starting our search on the highest level
/// of our list, dropping down a level when we reach a [Node](node::Node) with a
/// `key` larger than our search bound.
///
/// For further, and much more thorough explanation and derivation of skip list
/// and their invariant, see also:
/// - [Skip List CS.CMU](https://www.cs.cmu.edu/~ckingsf/bioinfo-lectures/skiplists.pdf)
/// - [Skip List Data Structure](https://www.mydistributed.systems/2021/03/skip-list-data-structure.html)
/// - [Skip List Proposal/Priority Queue](https://tstentz.github.io/418proposal/)
pub struct SkipList<K, V> {
    head: NonNull<Head<K, V>>,
    state: Padded<ListState>,
    incin: SharedIncin<K, V>,
}

make_shared_incin! {
    { "[`SkipList`]" }
    SharedIncin<K, V> of DeallocOnDrop<K, V>
}

impl<K, V> SkipList<K, V> {
    /// Create a new and empty [SkipList](SkipList).
    pub fn new() -> Self {
        SkipList {
            head: Head::new(),
            state: Padded::new(ListState::new()),
            incin: SharedIncin::new(),
        }
    }

    /// Returns the length of the [SkipList](SkipList). This is more of an
    /// estimate and there are no strong
    /// validity guarantees.
    pub fn len(&self) -> usize {
        match self.state.len.load(Ordering::Relaxed) {
            // Due to relaxed memory ordering, this may underflow at times.
            len if len < (isize::MAX as usize) => 0,
            len => len,
        }
    }

    /// Returns true if the [SkipList](SkipList) is *about* empty. Similar to
    /// [len](SkipList::len) this does not come with strong validity guarantees.
    pub fn is_empty(&self) -> bool {
        self.state.len.load(Ordering::Relaxed) < 1
    }

    /// Generates a random height for a [Node](node::Node) and updates the list
    /// seed.
    fn gen_height(&self) -> usize {
        let mut seed = self.state.seed.load(Ordering::Relaxed);
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;

        self.state.seed.store(seed, Ordering::Relaxed);

        let mut height =
            std::cmp::min(HEIGHT, seed.trailing_zeros() as usize + 1);

        let head = unsafe { &(*self.head.as_ptr()) };

        while height >= 4 && head.levels[height - 2].load_ptr().is_null() {
            height -= 1;
        }

        if height > self.state.max_height.load(Ordering::Relaxed) {
            self.state.max_height.store(height, Ordering::Relaxed);
        }

        height
    }
}

impl<K, V> SkipList<K, V>
where
    K: Ord + Send + Sync,
    V: Send + Sync,
{
    /// Inserts a value in the list given a key.
    pub fn insert<'a>(&'a self, key: K, val: V) -> Option<Entry<'a, K, V>> {
        // After this check, whether we are holding the head or a regular Node
        // will not impact the operation.
        let mut insertion_point = self.find(&key, false);
        let mut existing = None;

        while let Some(target) = insertion_point.target.take() {
            if target.try_remove_and_tag().is_ok() {
                existing = Some(target.clone());
                unsafe {
                    let _ = self.unlink(
                        target.clone(),
                        target.height(),
                        &insertion_point.prev,
                    );
                }
                insertion_point = self.find(&key, false);
            }
        }

        let mut prev = insertion_point.prev;

        let new_node_raw = Node::new_rand_height(key, val, self);

        // Protects the new_node so concurrent removals do not invalidate our
        // pointer.
        let new_node = self
            .node_ref_with(|| new_node_raw)
            .expect("new_node to not be null!");

        let mut starting_height = 0;

        self.state.len.fetch_add(1, Ordering::AcqRel);

        unsafe {
            while let Err(starting) =
                self.link_nodes(&new_node, prev, starting_height)
            {
                let mut search = self.find(&new_node.key, false);

                while let Some(target) = search.target.take() {
                    if core::ptr::eq(target.as_ptr(), new_node.as_ptr()) {
                        break;
                    }

                    if target.try_remove_and_tag().is_ok() {
                        existing = Some(target.clone());
                        let _ = self.unlink(
                            target.clone(),
                            target.height(),
                            &search.prev,
                        );
                        search = self.find(&new_node.key, false);
                    }
                }

                (starting_height, prev) = (starting, search.prev);
            }
        }

        existing.map(|existing| existing.into())
    }

    /// This function is unsafe, as it does not check whether new_node or link
    /// node are valid pointers.
    ///
    /// # Safety
    ///
    /// 1. `new_node` cannot be null
    /// 2. A tower of sufficient height must eventually be reached, the list
    /// head can be this tower
    unsafe fn link_nodes<'a>(
        &self,
        new_node: &'a NodeRef<'a, K, V>,
        previous_nodes: [NodeRef<'a, K, V>; HEIGHT],
        start_height: usize,
    ) -> Result<(), usize> {
        // iterate over all the levels in the new nodes pointer tower
        fence(Ordering::Release);

        for i in start_height .. new_node.height() {
            let prev = &previous_nodes[i];

            let next = self.node_ref_with(|| prev.levels[i].load_ptr());

            let next_ptr =
                next.as_ref().map_or(std::ptr::null_mut(), |n| n.as_ptr());

            let curr_next = new_node.levels[i].load_ptr();

            if new_node.removed() {
                break;
            }

            // We check if the next node is actually lower in key than our
            // current node. If the key is not greater we stop
            // building our node.
            match next.as_ref() {
                Some(next)
                    if next.key <= new_node.key && !new_node.removed() =>
                {
                    break
                },
                _ => (),
            };

            // Swap the previous' next node into the new_node's level
            // It could be the case that we link ourselves to the previous node,
            // but just as we do this `next` attempts to unlink
            // itself and fails. So while we succeeded, `next`
            // repeats its search and finds that we are the next
            if new_node.levels[i]
                .compare_exchange(
                    curr_next,
                    next_ptr,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_err()
            {
                return Err(i);
            };

            // If this is the base level, we simply increment the ref count, as
            // we expect it to be 0. If it is not, we only increment
            // if it > 0.
            if i == 0 {
                new_node.add_ref();
            } else if new_node.try_add_ref().is_err() {
                break;
            }

            // Swap the new_node into the previous' level. If the previous'
            // level has changed since the search, we repeat the
            // search from this level.
            if prev.levels[i]
                .compare_exchange(
                    next_ptr,
                    new_node.as_ptr(),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_err()
            {
                new_node.sub_ref();
                return Err(i);
            }
        }

        // IF we linked the node, yet it was removed during that process, there
        // may be some levels that we linked and that were missed by the
        // removers. We search to unlink those too.
        if new_node.removed() {
            self.find(&new_node.key, false);
        }

        Ok(())
    }

    /// Removes a key-value pair from the [SkipList](SkipList) if the given
    /// `key` is present and returns a protected *immutable* reference to the
    /// pair.
    pub fn remove<'a>(&'a self, key: &K) -> Option<Entry<'a, K, V>>
    where
        K: Send,
        V: Send,
    {
        match self.find(key, false) {
            SearchResult { target: Some(target), prev } => {
                // Set the target state to being removed
                // If this errors, it is already being removed by someone else
                // and thus we exit early.
                if target.set_removed().is_err() {
                    return None;
                }

                // # Safety:
                // 1. `key` and `val` will not be tempered with.
                // TODO This works for now, yet once `Atomic` is used
                // this may need to change.
                let height = target.height();

                if let Err(_) = target.tag_levels(1) {
                    panic!("SHOULD NOT BE TAGGED!")
                };

                // #Safety:
                // 1. The height we got from the `node` guarantees it is a valid
                // height for levels.
                unsafe {
                    if self.unlink(target.clone(), height, &prev).is_err() {
                        self.find(&key, false);
                    }
                }

                Some(target.into())
            },
            _ => None,
        }
    }

    /// Logically removes the node from the list by linking its adjacent nodes
    /// to one-another.
    ///
    /// # Safety
    /// 1. All indices in [0, height) are valid indices for `node.levels`.
    unsafe fn unlink<'a>(
        &self,
        mut node: NodeRef<'a, K, V>,
        height: usize,
        previous_nodes: &[NodeRef<'a, K, V>; HEIGHT],
    ) -> Result<(), usize> {
        // safety check against UB caused by unlinking the head
        if self.is_head(node.as_ptr()) {
            panic!()
        }

        // # Safety
        //
        // 1.-3. Some as method and covered by method caller.
        // 4. We are not unlinking the head. - Covered by previous safety check.
        for (i, prev) in previous_nodes.iter().enumerate().take(height).rev() {
            let (new_next, _tag) = node.levels[i].load_decomposed();

            // We check if the previous node is being removed after we have
            // already unlinked from it as the prev nodes expects us
            // to do this. We still need to stop the unlink here, as
            // we will have to relink to the actual, lively previous
            // node at this level as well.

            // Performs a compare_exchange, expecting the old value of the
            // pointer to be the current node. If it is not, we
            // cannot make any reasonable progress, so we search again.
            if prev.levels[i]
                .compare_exchange(
                    node.as_ptr(),
                    new_next,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_err()
            {
                return Err(i + 1);
            }

            node = if let Some(node) = self.sub_ref(node) {
                node
            } else {
                break;
            };
        }

        self.state.len.fetch_sub(1, Ordering::Relaxed);

        drop(previous_nodes);

        // we see if we can drop some pointers in the list.
        self.incin.inner.try_clear();
        Ok(())
    }

    /// Decrements the reference count of the `Node` by 1. If the reference
    /// count is thus 0, we retire the node.
    fn sub_ref<'a>(
        &self,
        node: NodeRef<'a, K, V>,
    ) -> Option<NodeRef<'a, K, V>> {
        if node.sub_ref() == 0 {
            let NodeRef { node, _pause, .. } = node;

            _pause.add_to_incin(DeallocOnDrop::from(node.as_ptr()));
            None
        } else {
            Some(node)
        }
    }

    /// Unlink [Node](Node) `curr` at the given level of [Node](Node) `prev` by
    /// exchanging the pointer for `next`.
    ///
    /// # Safety
    ///
    /// 1. `prev`, `curr`, are protected accesses.
    #[allow(unused)]
    unsafe fn unlink_level<'a>(
        &'a self,
        prev: &NodeRef<'a, K, V>,
        curr: NodeRef<'a, K, V>,
        next: Option<NodeRef<'a, K, V>>,
        level: usize,
    ) -> Result<Option<NodeRef<'a, K, V>>, ()> {
        // The pointer to `next` is tagged to signal unlinking.
        let next_ptr =
            next.as_ref().map_or(core::ptr::null_mut(), |n| n.as_ptr());

        if let Ok(_) = prev.levels[level].compare_exchange(
            curr.as_ptr(),
            next_ptr,
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            self.sub_ref(curr);

            Ok(next)
        } else {
            Err(())
        }
    }

    /// Find a `Node` given a `key`. If `search_closest` it may also return the
    /// next greater `Node` if the `key` is not present. Additionally, it
    /// returns an array holding the previous `Nodes` in the list that link
    /// to the target node.
    fn find<'a>(
        &'a self,
        key: &K,
        search_closest: bool,
    ) -> SearchResult<'a, K, V> {
        let head = unsafe { &(*self.head.as_ptr()) };

        // Initialize the `prev` array.
        let mut prev = unsafe {
            let mut prev: [core::mem::MaybeUninit<NodeRef<'a, K, V>>; HEIGHT] =
                core::mem::MaybeUninit::uninit().assume_init();

            for level in prev.iter_mut() {
                core::ptr::write(
                    level.as_mut_ptr(),
                    self.node_ref_with(|| {
                        self.head.cast::<Node<K, V>>().as_ptr()
                    })
                    .expect("Head to not be null!"),
                )
            }

            core::mem::transmute::<_, [NodeRef<'a, K, V>; HEIGHT]>(prev)
        };

        '_search: loop {
            let mut level = self.state.max_height.load(Ordering::Relaxed);
            // Find the first and highest node tower
            while level > 1 && head.levels[level - 1].load_ptr().is_null() {
                level -= 1;
            }

            // We need not protect the head, as it will always be valid, as long
            // as we are in a sane state.
            let mut curr = self
                .node_ref_with(|| self.head.as_ptr().cast::<Node<K, V>>())
                .expect("Head to not be null!");

            // steps:
            // 1. Go through each level until we reach a node with a key GEQ to
            // ours or that is null     1.1 If we are equal, then
            // the node must either be marked as removed or removed nodes
            //       are allowed in this search.
            //       Should this be the case, then we drop down a level while
            // also protecting a pointer       to the current node,
            // in order to keep the `Level` valid in our `prev` array.
            //     1.2 If we the `next` node is less or equal but removed and
            // removed nodes are       disallowed, then we set our
            // current node to the next node.
            while level > 0 {
                let next = unsafe {
                    let mut next = self
                        .node_ref_with(|| curr.levels[level - 1].load_ptr());

                    loop {
                        if next.is_none() {
                            break next;
                        }

                        if let Some(n) = next.as_ref() {
                            if n.levels[level - 1].load_tag() == 0 {
                                break next;
                            }
                        }

                        let n = next.unwrap();

                        let new_next = self
                            .node_ref_with(|| n.levels[level - 1].load_ptr());

                        let Ok(n) = self.unlink_level(&curr, n, new_next, level - 1) else {
                            continue '_search;
                        };

                        next = n
                    }
                };

                match next {
                    Some(next) if (*next).key < *key => {
                        prev[level - 1] = curr;

                        curr = next;
                    },
                    _ => {
                        // Update previous_nodes.
                        prev[level - 1] = curr.clone();

                        level -= 1;
                    },
                }
            }

            unsafe {
                return if search_closest {
                    let mut next =
                        self.node_ref_with(|| curr.levels[0].load_ptr());

                    loop {
                        if next.is_none() {
                            break;
                        }

                        if let Some(n) = next.as_ref() {
                            if n.levels[0].load_tag() == 0 {
                                break;
                            }
                        }

                        let n = next.unwrap();

                        let new_next =
                            self.node_ref_with(|| n.levels[0].load_ptr());

                        let Ok(n) = self.unlink_level(&curr, n, new_next, level - 1) else {
                            continue '_search;
                        };

                        next = n
                    }

                    SearchResult { prev, target: next }
                } else {
                    match self
                        .node_ref_with(|| prev[0].as_ref().levels[0].load_ptr())
                    {
                        Some(next) if next.key == *key && !next.removed() => {
                            SearchResult { prev, target: Some(next) }
                        },
                        _ => SearchResult { prev, target: None },
                    }
                };
            }
        }
    }

    /// Get a reference to an [Entry](Entry) if one with the given key exists.
    pub fn get<'a>(&'a self, key: &K) -> Option<Entry<'a, K, V>> {
        if self.is_empty() {
            return None;
        }

        // Perform safety check for whether we are dealing with the head.
        match self.find(key, false) {
            SearchResult { target: Some(target), .. } => {
                Some(Entry::from(target))
            },
            _ => None,
        }
    }

    fn is_head(&self, ptr: *const Node<K, V>) -> bool {
        std::ptr::eq(ptr, self.head.as_ptr().cast())
    }

    /// Returns the next [Node](Node) in the [SkipList](SkipList) if the given
    /// [Node](Node) is not the last.
    fn next_node<'a>(
        &'a self,
        node: &Entry<'a, K, V>,
    ) -> Option<Entry<'a, K, V>> {
        let node: &NodeRef<'_, _, _> = unsafe { core::mem::transmute(node) };

        // This means we have a stale node and cannot return a sane answer!
        if node.levels[0].load_tag() == 1 {
            return self.find(&node.key, true).target.map(|t| t.into());
        };

        let mut next = self.node_ref_with(|| node.levels[0].load_ptr())?;

        // Unlink and skip all removed `Node`s we may encounter.
        while next.levels[0].load_tag() == 1 {
            let new = self.node_ref_with(|| next.levels[0].load_ptr());
            next = unsafe {
                self.unlink_level(&node, next, new, 0)
                    .ok()
                    .unwrap_or_else(|| self.find(&node.key, true).target)?
            };
        }

        Some(next.into())
    }

    /// Returns the first [Node](Node) in the [SkipList](SkipList) if the list
    /// is not empty.
    pub fn get_first<'a>(&'a self) -> Option<Entry<'a, K, V>> {
        if self.is_empty() {
            return None;
        }

        let curr = self
            .node_ref_with(|| self.head.as_ptr().cast::<Node<K, V>>())
            .expect("Head to not be null");

        self.next_node(&curr.into())
    }

    /// Removes the first [Node](Node) (with the smallest key) from the list if
    /// it is not empty.
    pub fn pop_first<'a>(&'a self) -> Option<Entry<'a, K, V>> {
        self.get_first()?.remove()
    }

    /// Returns the last [Node](Node) in the [SkipList](SkipList) if the list
    /// is not empty. Runtime is `O(n)`
    pub fn get_last<'a>(&'a self) -> Option<Entry<'a, K, V>> {
        let mut curr = self.get_first()?;

        while let Some(next) = self.next_node(&curr) {
            curr = next;
        }

        return Some(curr.into());
    }

    /// Removes the first [Node](Node) (with the smallest key) from the list if
    /// it is not empty.
    pub fn pop_last<'a>(&'a self) -> Option<Entry<'a, K, V>> {
        self.get_last()?.remove()
    }

    /// Returns a borrowing iterator over the [SkipList](SkipList) that yields
    /// [Entries](Entry) into the list.
    pub fn iter<'a>(&'a self) -> iter::Iter<'a, K, V> {
        iter::Iter::from_list(self)
    }

    fn node_ref_with<F>(&self, f: F) -> Option<NodeRef<'_, K, V>>
    where
        F: FnOnce() -> *mut Node<K, V>,
    {
        NodeRef::from_pause_with_in(self.incin.inner.pause(), self, f)
    }
}

impl<K, V> Default for SkipList<K, V>
where
    K: Sync,
    V: Sync,
{
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<K, V> Send for SkipList<K, V>
where
    K: Send + Sync,
    V: Send + Sync,
{
}

unsafe impl<K, V> Sync for SkipList<K, V>
where
    K: Send + Sync,
    V: Send + Sync,
{
}

// TODO Verify this is sound for all variants of SkipList
/// Manual `Drop` implementation for all `SkipList`s
impl<K, V> Drop for SkipList<K, V> {
    fn drop(&mut self) {
        // To ensure this is safe, clear all `HazardPointer`s in the
        // domain. We do not want to drop a node twice!
        self.incin.clear();
        let mut node = unsafe { (*self.head.as_ptr()).levels[0].load_ptr() };

        // # Safety
        //
        // We have an exclusive reference to `SkipList`.
        unsafe {
            while !node.is_null() {
                let temp = node;
                node = (*temp).levels[0].load_ptr();
                Node::<K, V>::drop(temp);
            }

            Head::<K, V>::drop(self.head);
        }
    }
}

impl<K, V> Debug for SkipList<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkipList").field("head", &self.head.as_ptr()).finish()
    }
}

/// Frequently accessed data in the [SkipList](SkipList).
struct ListState {
    len: AtomicUsize,
    max_height: AtomicUsize,
    seed: AtomicUsize,
}

impl ListState {
    fn new() -> Self {
        ListState {
            len: AtomicUsize::new(0),
            max_height: AtomicUsize::new(1),
            seed: AtomicUsize::new(rand::random()),
        }
    }
}

/// A protected and *shared* reference to a key-value pair from or in the
/// [SkipList](SkipList).
#[repr(C)]
pub struct Entry<'a, K, V> {
    node: core::ptr::NonNull<Node<K, V>>,
    list: &'a SkipList<K, V>,
    _pause: crate::incin::Pause<'a, DeallocOnDrop<K, V>>,
}

impl<'a, K, V> Entry<'a, K, V> {
    /// Returns the value of the key-value pair.
    pub fn val(&self) -> &V {
        // #Safety
        //
        // Our `HazardPointer` ensures that our pointers is valid.
        unsafe { &self.node.as_ref().val }
    }

    /// Returns the key of the key-value pair.
    pub fn key(&self) -> &K {
        // #Safety
        //
        // Our `HazardPointer` ensures that our pointers is valid.
        unsafe { &self.node.as_ref().key }
    }
}
impl<'a, K, V> Entry<'a, K, V>
where
    K: Ord + Send + Sync,
    V: Send + Sync,
{
    /// Removes the [Entry](Entry) from the [SkipList](SkipList) if
    /// it is not already removed.
    pub fn remove(self) -> Option<Entry<'a, K, V>> {
        unsafe {
            self.node.as_ref().set_removed().ok()?;

            self.node.as_ref().tag_levels(1).expect("no tags to exists");

            self.list.find(&self.key, false);

            Some(self)
        }
    }
}

impl<'a, K, V> core::ops::Deref for Entry<'a, K, V> {
    type Target = Node<K, V>;

    fn deref(&self) -> &Self::Target {
        unsafe { self.node.as_ref() }
    }
}

struct SearchResult<'a, K, V> {
    prev: [NodeRef<'a, K, V>; HEIGHT],
    target: Option<NodeRef<'a, K, V>>,
}

impl<'a, K, V> Debug for SearchResult<'a, K, V>
where
    K: Debug + Default,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchResult").field("target", &self.target).finish()
    }
}

#[repr(C)]
struct NodeRef<'a, K, V> {
    node: NonNull<Node<K, V>>,
    list: &'a SkipList<K, V>,
    _pause: crate::incin::Pause<'a, DeallocOnDrop<K, V>>,
}

impl<'a, K, V> NodeRef<'a, K, V> {
    fn from_pause_with_in<F>(
        pause: crate::incin::Pause<'a, DeallocOnDrop<K, V>>,
        list: &'a SkipList<K, V>,
        f: F,
    ) -> Option<Self>
    where
        F: FnOnce() -> *mut Node<K, V>,
    {
        let ptr = f();
        unsafe {
            if !ptr.is_null() {
                Some(NodeRef {
                    node: NonNull::new_unchecked(ptr),
                    list,
                    _pause: pause,
                })
            } else {
                None
            }
        }
    }

    fn from_raw_and_pause(
        list: &'a SkipList<K, V>,
        pause: crate::incin::Pause<'a, DeallocOnDrop<K, V>>,
        raw: *mut Node<K, V>,
    ) -> NodeRef<'a, K, V> {
        unsafe {
            NodeRef { node: NonNull::new_unchecked(raw), list, _pause: pause }
        }
    }

    fn as_ptr(&self) -> *mut Node<K, V> {
        self.node.as_ptr()
    }
}

impl<'a, K, V> AsRef<Node<K, V>> for NodeRef<'a, K, V> {
    fn as_ref(&self) -> &Node<K, V> {
        unsafe { &(*self.as_ptr()) }
    }
}

impl<'a, K, V> core::ops::Deref for NodeRef<'a, K, V> {
    type Target = Node<K, V>;
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<'a, K, V> core::ops::DerefMut for NodeRef<'a, K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut (*self.as_ptr()) }
    }
}

impl<'a, K, V> core::fmt::Debug for NodeRef<'a, K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe {
            f.debug_struct("NodeRef").field("node", self.node.as_ref()).finish()
        }
    }
}

impl<'a, K, V> From<NodeRef<'a, K, V>> for Entry<'a, K, V> {
    fn from(value: NodeRef<'a, K, V>) -> Self {
        unsafe { core::mem::transmute(value) }
    }
}

impl<'a, K, V> Clone for NodeRef<'a, K, V> {
    fn clone(&self) -> Self {
        NodeRef {
            node: self.node.clone(),
            list: self.list,
            _pause: self._pause.clone(),
        }
    }
}

impl<'a, K, V> core::cmp::PartialEq for NodeRef<'a, K, V> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.node.as_ptr(), other.node.as_ptr())
    }
}

#[repr(transparent)]
struct DeallocOnDrop<K, V>(*mut Node<K, V>);

unsafe impl<K, V> Send for DeallocOnDrop<K, V>
where
    K: Send + Sync,
    V: Send + Sync,
{
}

unsafe impl<K, V> Sync for DeallocOnDrop<K, V>
where
    K: Send + Sync,
    V: Send + Sync,
{
}

impl<K, V> From<*mut Node<K, V>> for DeallocOnDrop<K, V> {
    fn from(node: *mut Node<K, V>) -> Self {
        DeallocOnDrop(node)
    }
}

impl<K, V> Drop for DeallocOnDrop<K, V> {
    fn drop(&mut self) {
        unsafe { Node::drop(self.0) }
    }
}

impl<K, V> core::ops::Deref for DeallocOnDrop<K, V> {
    type Target = Node<K, V>;

    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.0) }
    }
}

impl<K, V> core::ops::DerefMut for DeallocOnDrop<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut (*self.0) }
    }
}

/// A lock-free binary search tree that that currently only supports concurrent
/// pushing with removal for now only working through a mutable reference.

pub mod iter {
    use super::Node;

    use super::{Entry, SkipList};
    use std::iter::FromIterator;

    /// A borrowing [Iterator](std::iter::Iterator) over [Entries](Entry) in the
    /// SkipList.
    pub struct Iter<'a, K, V> {
        list: &'a SkipList<K, V>,
        next: Option<Entry<'a, K, V>>,
    }

    impl<'a, K, V> Iter<'a, K, V>
    where
        K: Ord + Send + Sync,
        V: Send + Sync,
    {
        /// Creates an instance of [Iter](Iter) from a [SkipList](SkipList).
        pub fn from_list(list: &'a SkipList<K, V>) -> Self {
            Self { list, next: list.get_first() }
        }
    }

    impl<'a, K, V> core::iter::Iterator for Iter<'a, K, V>
    where
        K: Ord + Send + Sync,
        V: Send + Sync,
    {
        type Item = Entry<'a, K, V>;
        fn next(&mut self) -> Option<Self::Item> {
            if let Some(next) = self.next.take() {
                self.next = self.list.next_node(&next);
                return Some(next);
            }

            None
        }
    }

    impl<K, V> IntoIterator for SkipList<K, V>
    where
        K: Ord + Send + Sync,
        V: Send + Sync,
    {
        type Item = (K, V);
        type IntoIter = IntoIter<K, V>;
        fn into_iter(self) -> Self::IntoIter {
            IntoIter::from_list(self)
        }
    }

    impl<K, V> FromIterator<(K, V)> for SkipList<K, V>
    where
        K: Ord + Send + Sync,
        V: Send + Sync,
    {
        fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
            let list = Self::new();
            for (k, v) in iter {
                list.insert(k, v);
            }

            list
        }
    }

    /// An owning [Iterator](std::iter::Iterator) over key-value pairs from
    /// a [SkipList](SkipList).
    pub struct IntoIter<K, V> {
        next: *mut Node<K, V>,
    }

    impl<K, V> IntoIter<K, V>
    where
        K: Ord + Send + Sync,
        V: Send + Sync,
    {
        /// Creates an instance of [IntoIter] from a [SkipList](SkipList).
        pub fn from_list<'a>(mut list: SkipList<K, V>) -> Self {
            unsafe {
                let next = list.head.as_ref().levels[0].load_ptr();
                for level in list.head.as_mut().levels.pointers.iter_mut() {
                    level.store_ptr(core::ptr::null_mut());
                }

                IntoIter { next }
            }
        }
    }

    impl<K, V> core::iter::Iterator for IntoIter<K, V>
    where
        K: Ord + Send + Sync,
        V: Send + Sync,
    {
        type Item = (K, V);
        fn next(&mut self) -> Option<Self::Item> {
            if self.next.is_null() {
                return None;
            }

            let next = self.next;

            self.next = unsafe { (*next).levels[0].load_ptr() };

            let (key, val) = unsafe {
                (core::ptr::read(&(*next).key), core::ptr::read(&(*next).val))
            };

            unsafe {
                Node::dealloc(next);
            }

            (key, val).into()
        }
    }
}

#[cfg(test)]
mod skiplist_test {
    use self::rand::Rng;
    use super::*;

    #[test]
    fn test_new_node_sync() {
        let node = Node::new(100, "hello", 1);
        let other = Node::new(100, "hello", 1);
        unsafe { println!("node 1: {:?},", *node) };
        unsafe { println!("node 2: {:?},", *other) };
        let other = unsafe {
            let node = Node::alloc(1);
            core::ptr::write(&mut (*node).key, 100);
            core::ptr::write(&mut (*node).val, "hello");
            node
        };

        unsafe { println!("node 1: {:?}, node 2: {:?}", *node, *other) };

        unsafe { assert_eq!(*node, *other) };
    }

    #[test]
    fn test_new_list_sync() {
        let _: SkipList<usize, usize> = SkipList::new();
    }

    #[test]
    fn test_insert_sync() {
        let list = SkipList::new();
        let mut rng: u16 = rand::random();

        for _ in 0 .. 10_000 {
            rng ^= rng << 3;
            rng ^= rng >> 12;
            rng ^= rng << 7;
            list.insert(rng, "hello there!");
        }
    }

    #[test]
    fn test_rand_height_sync() {
        let list: SkipList<&str, &str> = SkipList::new();
        let node = Node::new_rand_height("Hello", "There!", &list);

        assert!(!node.is_null());
        let height = unsafe { (*node).levels.pointers.len() };

        println!("height: {}", height);

        unsafe {
            println!("{}", *node);
        }

        unsafe {
            let _ = Box::from_raw(node);
        }
    }

    #[test]
    fn test_drop() {
        struct CountOnDrop<K> {
            key: K,
            counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        }

        impl<K> CountOnDrop<K> {
            fn new(
                key: K,
                counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
            ) -> Self {
                CountOnDrop { key, counter }
            }

            fn new_none(key: K) -> Self {
                CountOnDrop {
                    key,
                    counter: std::sync::Arc::new(
                        std::sync::atomic::AtomicUsize::new(0),
                    ),
                }
            }
        }
        impl<K: Eq> PartialEq for CountOnDrop<K> {
            fn eq(&self, other: &Self) -> bool {
                self.key == other.key
            }
        }

        impl<K: Eq> Eq for CountOnDrop<K> {}

        impl<K: Ord> PartialOrd for CountOnDrop<K> {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.key.cmp(&other.key))
            }
        }

        impl<K: Ord> Ord for CountOnDrop<K> {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.key.cmp(&other.key)
            }
        }

        impl<K> Drop for CountOnDrop<K> {
            fn drop(&mut self) {
                println!("writing to counter!");
                println!("count: {}", self.counter.load(Ordering::SeqCst));
                self.counter.fetch_add(1, Ordering::SeqCst);
                println!("wrote to counter");
            }
        }

        let counter =
            std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let list = SkipList::new();

        list.insert(CountOnDrop::new(1, counter.clone()), ());

        list.remove(&CountOnDrop::new_none(1));

        // assert_eq!(counter.load(Ordering::SeqCst), 1);

        list.insert(CountOnDrop::new(1, counter.clone()), ());

        list.insert(CountOnDrop::new(1, counter.clone()), ());

        println!("length: {}", list.len());

        list.incin.inner.try_clear();

        core::sync::atomic::fence(Ordering::SeqCst);

        assert_eq!(counter.load(Ordering::SeqCst), 2);

        drop(list);

        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_insert_verbose_sync() {
        let list = SkipList::new();

        list.insert(1, 1);

        list.iter().for_each(|n| println!("k: {},", n.key()));

        list.insert(2, 2);

        list.iter().for_each(|n| println!("k: {},", n.key()));

        list.insert(5, 3);

        list.iter().for_each(|n| println!("k: {},", n.key()));
    }

    #[test]
    fn test_remove() {
        let list = SkipList::new();
        let mut rng: u16 = rand::random();

        for _ in 0 .. 10_000 {
            rng ^= rng << 3;
            rng ^= rng >> 12;
            rng ^= rng << 7;
            list.insert(rng, "hello there!");
        }
        for _ in 0 .. 10_000 {
            rng ^= rng << 3;
            rng ^= rng >> 12;
            rng ^= rng << 7;
            list.remove(&rng);
        }
    }

    #[test]
    fn test_verbose_remove() {
        let list = SkipList::new();

        list.insert(1, 1);
        list.insert(2, 2);
        list.insert(2, 2);
        list.insert(5, 3);

        list.iter().for_each(|n| println!("k: {},", n.key()));

        assert!(list.remove(&1).is_some());

        list.iter().for_each(|n| println!("k: {},", n.key()));

        println!("removing 6");
        assert!(list.remove(&6).is_none());
        println!("removing 1");
        assert!(list.remove(&1).is_none());
        println!("removing 5");
        assert!(list.remove(&5).is_some());
        println!("removing 2");
        assert!(list.remove(&2).is_some());

        list.iter().for_each(|n| println!("k: {},", n.key()));

        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_find_removed() {
        let list = SkipList::new();

        list.insert(3, ());

        list.insert(4, ());

        list.insert(5, ());

        assert!(list.find(&3, false).target.is_some());
        assert!(list.find(&4, false).target.is_some());

        // manually get reference to the nodes
        let node_3 =
            unsafe { &mut (*(*list.head.as_ptr()).levels[0].load_ptr()) };
        let node_4 = unsafe {
            &mut (*(*(*list.head.as_ptr()).levels[0].load_ptr()).levels[0]
                .load_ptr())
        };
        let node_5 = unsafe {
            &mut (*(*(*(*list.head.as_ptr()).levels[0].load_ptr()).levels[0]
                .load_ptr())
            .levels[0]
                .load_ptr())
        };

        // make sure it is the right node
        assert_eq!(node_3.key, 3);
        println!("{:?}", node_3);
        assert_eq!(node_4.key, 4);
        println!("{:?}", node_4);
        assert_eq!(node_5.key, 5);
        println!("{:?}", node_5);

        // remove the node logically
        let _ = node_4.set_removed();

        assert!(list.find(&4, false).target.is_none());

        println!("{:?}", list.find(&3, false));

        assert!(!node_3.removed());

        assert!(list.remove(&4).is_none());

        // remove the node logically
        node_4.height_and_removed.store(
            node_4.height_and_removed.load(Ordering::SeqCst)
                & (usize::MAX >> 1),
            Ordering::SeqCst,
        );

        assert!(!node_4.removed());

        assert!(list.remove(&4).is_some());
    }

    #[test]
    fn test_sync_remove() {
        use std::sync::Arc;
        let list = Arc::new(SkipList::new());
        let mut rng = rand::thread_rng();

        for _ in 0 .. 10_000 {
            list.insert(rng.gen::<u16>(), ());
        }
        let threads = (0 .. 20)
            .map(|_| {
                let list = list.clone();
                std::thread::spawn(move || {
                    let mut rng = rand::thread_rng();
                    for _ in 0 .. 1_000 {
                        let target = &rng.gen::<u16>();
                        list.remove(&target);
                    }
                })
            })
            .collect::<Vec<_>>();

        for thread in threads {
            thread.join().unwrap()
        }

        list.iter().for_each(|e| println!("key: {}", e.key));
    }

    #[test]
    fn test_sync_insert() {
        use std::sync::Arc;
        let list = Arc::new(SkipList::new());

        let threads = (0 .. 20)
            .map(|_| {
                let list = list.clone();
                std::thread::spawn(move || {
                    let mut rng = rand::thread_rng();
                    for _ in 0 .. 1_000 {
                        let target = rng.gen::<u8>();

                        list.insert(target, ());
                    }
                })
            })
            .collect::<Vec<_>>();

        for thread in threads {
            thread.join().unwrap()
        }

        list.iter().for_each(|e| println!("key: {}", e.key));
    }

    #[test]
    fn test_sync_inmove() {
        use std::sync::Arc;
        let list = Arc::new(SkipList::new());

        let threads = (0 .. 20)
            .map(|_| {
                let list = list.clone();
                std::thread::spawn(move || {
                    let mut rng = rand::thread_rng();
                    for _ in 0 .. 5_000 {
                        let target = rng.gen::<u8>();
                        if rng.gen::<u8>() % 5 == 0 {
                            list.remove(&target);
                        } else {
                            list.insert(target, ());
                        }
                    }
                })
            })
            .collect::<Vec<_>>();

        for thread in threads {
            thread.join().unwrap()
        }

        list.iter().for_each(|e| println!("key: {}", e.key));
    }

    #[test]
    fn test_sync_iterate() {
        use std::sync::Arc;
        let list = Arc::new(SkipList::new());

        let threads = (0 .. 20)
            .map(|_| {
                let list = list.clone();
                std::thread::spawn(move || {
                    let mut rng = rand::thread_rng();
                    for _ in 0 .. 1_000 {
                        let target = rng.gen::<u8>();
                        if rng.gen::<u8>() % 5 == 0 {
                            list.remove(&target);
                        } else {
                            list.insert(target, ());
                        }
                    }
                })
            })
            .collect::<Vec<_>>();

        for _ in 0 .. 5 {
            list.iter().for_each(|e| println!("key: {}", e.key()));
        }

        for thread in threads {
            thread.join().unwrap()
        }

        let list = Arc::<SkipList<u8, ()>>::try_unwrap(list).unwrap();

        list.into_iter().for_each(|(k, _)| println!("key: {}", k))
    }
}
