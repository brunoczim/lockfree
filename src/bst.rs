use std::{
    cmp::Ordering::{Equal, Greater, Less},
    fmt::Debug,
    ptr::{self, NonNull},
    sync::atomic::{AtomicPtr, Ordering},
    vec,
};

use owned_alloc::OwnedAlloc;

use crate::removable::Removable;

/// A lock-free binary search tree that that currently only supports concurrent
/// pushing with removal for now only working through a mutable reference.
pub struct BSTree<K, V> {
    head: AtomicPtr<TreeNode<K, V>>,
    incin: SharedIncin<K, V>,
}

make_shared_incin! {
    { "[`BSTree`]" }
    pub SharedIncin<K, V> of OwnedAlloc<TreeNode<K, V>>
}

impl<K, V> BSTree<K, V> {
    /// Creates a new empty binary search tree.
    pub fn new() -> BSTree<K, V> {
        BSTree {
            head: AtomicPtr::new(ptr::null_mut()),
            incin: SharedIncin::default(),
        }
    }
}

impl<K: Ord, V> BSTree<K, V> {
    /// Inserts a new key-value pair into the tree. If a value with the same key
    /// already exists it returns the old key-value pair.
    pub fn insert(&self, key: K, value: V) -> Option<V> {
        let value = Box::into_raw(Box::new(value));

        // This may cause memory leaks.
        // Would need more testing.
        let f = move |_: Option<&V>| unsafe { ptr::read(value) };
        self.insert_with_optional(key, f)
    }

    /// Inserts a key value pair given a closure that produces the value. If the
    /// key already exists, the closure is passed a reference to that
    /// value. If not, the closure is passed the default value.
    pub fn insert_with<F>(&self, key: K, f: F) -> Option<V>
    where
        F: Fn(&V) -> V,
        V: Default,
    {
        let g = |v: Option<&V>| match v {
            Some(value) => f(value),
            None => f(&V::default()),
        };

        self.insert_with_optional(key, g)
    }

    /// Inserts a key value pair given a closure. If the key already exists,
    /// the closure is passed reference to it, if not it is passed `None`.
    pub fn insert_with_optional<F>(&self, key: K, f: F) -> Option<V>
    where
        F: Fn(Option<&V>) -> V,
    {
        let mut curr_ptr = self.head.load(Ordering::Acquire);
        let alloc = OwnedAlloc::new(TreeNode::new(key, f(None)));
        let new_node = alloc.into_raw().as_ptr();

        loop {
            if curr_ptr.is_null() {
                match self.head.compare_exchange(
                    curr_ptr,
                    new_node,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return None,
                    Err(other) => curr_ptr = other,
                }
            } else {
                let (current_ref, new_key) =
                    unsafe { (&*curr_ptr, &(*new_node).key) };

                // Compares the key of the new node to the current nodes key
                match new_key.cmp(&current_ref.key) {
                    Less => {
                        let left = current_ref.left.load(Ordering::Acquire);

                        if left.is_null() {
                            if let Ok(_) = current_ref.left.compare_exchange(
                                left,
                                new_node,
                                Ordering::AcqRel,
                                Ordering::Acquire,
                            ) {
                                return None;
                            }
                        } else {
                            curr_ptr = left;
                        }
                    },
                    Greater => {
                        let right = current_ref.right.load(Ordering::Acquire);
                        if right.is_null() {
                            if let Ok(_) = current_ref.right.compare_exchange(
                                right,
                                new_node,
                                Ordering::AcqRel,
                                Ordering::Acquire,
                            ) {
                                return None;
                            }
                        } else {
                            curr_ptr = right;
                        }
                    },
                    Equal => {
                        unsafe {
                            let new_value = (*curr_ptr)
                                .value
                                .get_mut()
                                .map(|old_value| f(Some(old_value)))
                                .unwrap_or_else(|| f(None));
                            return (*curr_ptr).value.replace(Some(new_value));
                        };
                    },
                }
            }
        }
    }

    /// Traverses the tree in sorted order and returns an iterator of owned
    /// values.
    pub fn order_traversal(&self) -> impl std::iter::Iterator<Item = V>
    where
        K: Clone,
        V: Clone,
    {
        let mut kvps = vec![];
        let f = |value: &V| kvps.push(value.clone());
        self.traverse_with(f);
        kvps.into_iter()
    }

    /// Traverse the tree in sorted order with a closure that is passed mutable
    /// reference to each key.
    pub fn traverse_with<F>(&self, f: F)
    where
        F: FnMut(&V),
    {
        // travereses the tree recursivel
        // recursivity is preferable here, so the underlying value does not get
        // dropped while we hold it.
        fn traverse_and_collect<K, V, F>(
            node: *mut TreeNode<K, V>,
            mut f: F,
        ) -> F
        where
            F: FnMut(&V),
        {
            if !node.is_null() {
                // Safety: we check our reference is not null while we are
                // holding it.
                let node = unsafe { &mut *node };
                f = traverse_and_collect(node.left.load(Ordering::SeqCst), f);
                node.value.get_mut().map(|value| f(value));
                f = traverse_and_collect(node.right.load(Ordering::SeqCst), f);
            }
            f
        }

        let curr_ptr = self.head.load(Ordering::Relaxed);

        traverse_and_collect(curr_ptr, f);
    }

    /// Drains all elements of the tree and returns them sorted in an iterator.
    pub fn drain(&mut self) -> impl std::iter::Iterator<Item = (K, V)> {
        let head = loop {
            let head = self.head.load(Ordering::Relaxed);
            match self.head.compare_exchange(
                head,
                ptr::null_mut(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(head) => break head,
                Err(_) => (),
            }
        };

        let mut stack: Vec<*mut TreeNode<K, V>> = vec![head];

        let mut kvps: Vec<(K, V)> = vec![];

        while stack.len() > 0 {
            if let Some(node_ptr) = stack.pop() {
                if !node_ptr.is_null() {
                    unsafe {
                        let TreeNode { key, value, left, right } =
                            *Box::from_raw(node_ptr);
                        if left.load(Ordering::Relaxed).is_null() {
                            value
                                .take(Ordering::AcqRel)
                                .map(|val| kvps.push((key, val)));

                            stack.push(right.load(Ordering::Relaxed));
                        } else {
                            let node_ptr = Box::into_raw(Box::new(TreeNode {
                                key,
                                value,
                                left: AtomicPtr::new(ptr::null_mut()),
                                right,
                            }));
                            stack.push(node_ptr);
                            stack.push(left.load(Ordering::Relaxed));
                        }
                    }
                    continue;
                }
            }
        }

        kvps.into_iter()
    }

    /// Verifies wether a given key with a value exists in the tree.
    pub fn contains(&self, key: &K) -> bool {
        match self.find(key) {
            Some(node) => unsafe {
                node.as_ref().value.is_present(Ordering::AcqRel)
            },
            None => false,
        }
    }

    /// Remove a node given a key. If the node exists it returns the underlying
    /// value, otherwise it returns `None`.
    pub fn remove(&self, key: &K) -> Option<V> {
        match self.find(key) {
            Some(node) => unsafe { node.as_ref().value.take(Ordering::AcqRel) },
            None => None,
        }
    }

    fn find(&self, key: &K) -> Option<NonNull<TreeNode<K, V>>> {
        let mut curr_ptr = self.head.load(Ordering::Acquire);
        while !curr_ptr.is_null() {
            let current_ref = unsafe { &*curr_ptr };

            // Compares the key of the new node to the current nodes key
            match key.cmp(&current_ref.key) {
                Less => curr_ptr = current_ref.left.load(Ordering::Acquire),
                Greater => curr_ptr = current_ref.right.load(Ordering::Acquire),
                Equal => break,
            }
        }
        NonNull::new(curr_ptr)
    }
}

impl<K, V> Default for BSTree<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Debug for BSTree<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let head = self.head.load(Ordering::Relaxed);
        if !head.is_null() {
            unsafe { write!(f, "BSTree {{ {:?} }}", *head) }
        } else {
            write!(f, "BSTree {{}}")
        }
    }
}

impl<K, V> Drop for BSTree<K, V> {
    fn drop(&mut self) {
        let mut stack: Vec<*mut TreeNode<K, V>> =
            vec![self.head.load(Ordering::Relaxed)];

        while let Some(node_ptr) = stack.pop() {
            if !node_ptr.is_null() {
                unsafe {
                    stack.push((*node_ptr).left.load(Ordering::Relaxed));
                    stack.push((*node_ptr).right.load(Ordering::Relaxed));
                    let _ = Box::from_raw(node_ptr);
                }
            }
        }
    }
}

unsafe impl<K, V> Send for BSTree<K, V>
where
    K: Send,
    V: Send,
{
}

unsafe impl<K, V> Sync for BSTree<K, V>
where
    K: Sync,
    V: Sync,
{
}

/// The borrowing iterator for BSTree
struct Iterator<K, V> {
    tree: BSTree<K, V>,
}

#[repr(align(2))]
struct TreeNode<K, V> {
    key: K,
    value: Removable<V>,
    left: AtomicPtr<TreeNode<K, V>>,
    right: AtomicPtr<TreeNode<K, V>>,
}

impl<K: Ord, V> TreeNode<K, V> {
    fn new(key: K, value: V) -> TreeNode<K, V> {
        TreeNode {
            key,
            value: Removable::new(value),
            left: AtomicPtr::new(ptr::null_mut()),
            right: AtomicPtr::new(ptr::null_mut()),
        }
    }

    fn with_ptrs(
        key: K,
        value: V,
        left: *mut TreeNode<K, V>,
        right: *mut TreeNode<K, V>,
    ) -> TreeNode<K, V> {
        TreeNode {
            key,
            value: Removable::new(value),
            left: AtomicPtr::new(left),
            right: AtomicPtr::new(right),
        }
    }
}

impl<K, V> Debug for TreeNode<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (left, right) = (
            self.left.load(Ordering::Relaxed),
            self.right.load(Ordering::Relaxed),
        );
        unsafe {
            match (left.is_null(), right.is_null()) {
                (true, true) => write!(
                    f,
                    "TreeNode {{ key: {:?}, value: {:?}, left: {{}}, right: \
                     {{}} }}",
                    self.key, self.value
                ),
                (false, true) => write!(
                    f,
                    "TreeNode {{ key: {:?}, value: {:?}, left: {:?}, right: \
                     {{}} }}",
                    self.key, self.value, *left
                ),
                (true, false) => write!(
                    f,
                    "TreeNode {{ key: {:?}, value: {:?} , left: {{}}, right: \
                     {:?} }}",
                    self.key, self.value, *right
                ),
                (false, false) => write!(
                    f,
                    "TreeNode {{ key: {:?}, value: {:?} , left: {:?}, right: \
                     {:?} }}",
                    self.key, self.value, *left, *right
                ),
            }
        }
    }
}

#[cfg(test)]
mod bst_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn insert_works() {
        let tree = BSTree::<i32, String>::new();
        tree.insert(1, "ffs".into());
        tree.insert(4, "203".into());

        tree.insert(3, "hello".into());
        tree.insert(3, "there".into());
        tree.insert(1, "yess".into());

        println!("{:?}", tree);

        println!("{:?}", tree.order_traversal().collect::<Vec<String>>());
    }

    #[derive(Debug)]
    struct CountOnDrop {
        counter: *const AtomicUsize,
    }

    impl CountOnDrop {
        fn from_ref(counter: &AtomicUsize) -> Self {
            Self { counter }
        }
    }

    unsafe impl Send for CountOnDrop {}
    unsafe impl Sync for CountOnDrop {}

    impl Drop for CountOnDrop {
        fn drop(&mut self) {
            unsafe {
                (*self.counter).fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    #[test]
    fn no_leak_drop() {
        let drop_count = AtomicUsize::new(0);
        let tree = BSTree::<i32, CountOnDrop>::new();
        tree.insert(1, CountOnDrop::from_ref(&drop_count));
        tree.insert(2, CountOnDrop::from_ref(&drop_count));
        tree.insert(5, CountOnDrop::from_ref(&drop_count));
        tree.insert(10, CountOnDrop::from_ref(&drop_count));
        tree.insert(3, CountOnDrop::from_ref(&drop_count));
        tree.insert(10, CountOnDrop::from_ref(&drop_count));

        assert_eq!(drop_count.load(Ordering::Acquire), 1);

        drop(tree);
        assert_eq!(drop_count.load(Ordering::Acquire), 6);
    }

    #[test]
    fn drain() {
        let mut tree = BSTree::<i32, i32>::new();
        let expected = vec![(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)];

        tree.insert(3, 3);
        tree.insert(5, 5);
        tree.insert(1, 1);
        tree.insert(4, 4);
        tree.insert(2, 2);

        println!("{:?}", tree);

        let actual = tree.drain().collect::<Vec<(i32, i32)>>();

        println!("{:?}", actual);

        assert_eq!(actual, expected);
    }

    #[test]
    fn threads_no_leaks() {
        use std::sync::Arc;

        let tree = Arc::new(BSTree::<i32, CountOnDrop>::new());
        let drop_counter = Arc::new(AtomicUsize::new(0));

        let mut threads = Vec::with_capacity(16);
        for _ in 0 .. 16 {
            let tree = tree.clone();
            let dc = drop_counter.clone();
            threads.push(std::thread::spawn(move || {
                for i in 0 .. 1_000 {
                    tree.insert(i % 32, CountOnDrop { counter: &*dc });
                }
            }))
        }

        threads.into_iter().for_each(|thread| thread.join().unwrap());

        println!(
            "{:?}",
            Arc::try_unwrap(tree)
                .unwrap()
                .drain()
                .map(|(k, _)| k)
                .collect::<Vec<i32>>()
        );

        assert_eq!(drop_counter.load(Ordering::SeqCst), 16_000);
    }
}
