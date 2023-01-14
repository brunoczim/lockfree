use super::{
    alloc::alloc::{alloc, dealloc, handle_alloc_error, Layout},
    tagged::MaybeTagged,
    SkipList,
    HEIGHT,
    HEIGHT_BITS,
    HEIGHT_MASK,
};

const REMOVED_MASK: usize = !(usize::MAX >> 1);

use std::{
    fmt::{Debug, Display},
    mem,
    ops::Index,
    ptr::{self, NonNull},
    sync::atomic::{AtomicUsize, Ordering},
};

/// Head stores the first pointer tower at the beginning of the list. It is
/// always of maximum
#[repr(C)]
pub(super) struct Head<K, V> {
    key: K,
    val: V,
    height_and_removed: AtomicUsize,
    pub(super) levels: Levels<K, V>,
}

impl<K, V> Head<K, V> {
    pub(super) fn new() -> NonNull<Self> {
        let head_ptr = unsafe { Node::<K, V>::alloc(super::HEIGHT).cast() };

        if let Some(head) = NonNull::new(head_ptr) {
            head
        } else {
            panic!()
        }
    }

    pub(super) unsafe fn drop(ptr: NonNull<Self>) {
        Node::<K, V>::dealloc(ptr.as_ptr().cast());
    }
}

#[repr(C)]
pub(super) struct Levels<K, V> {
    pub(super) pointers: [MaybeTagged<Node<K, V>>; 1],
}

impl<K, V> Levels<K, V> {
    fn get_size(height: usize) -> usize {
        assert!(height <= HEIGHT && height > 0);

        mem::size_of::<Self>() * (height - 1)
    }
}

impl<K, V> Index<usize> for Levels<K, V> {
    type Output = MaybeTagged<Node<K, V>>;

    fn index(&self, index: usize) -> &Self::Output {
        unsafe { self.pointers.get_unchecked(index) }
    }
}

#[repr(C)]
pub struct Node<K, V> {
    pub key: K,
    pub val: V,
    pub(super) height_and_removed: AtomicUsize,
    pub(super) levels: Levels<K, V>,
}

impl<K, V> Node<K, V> {
    pub(super) fn new(key: K, val: V, height: usize) -> *mut Self {
        unsafe {
            let node = Self::alloc(height);
            ptr::write(&mut (*node).key, key);
            ptr::write(&mut (*node).val, val);
            node
        }
    }

    pub(super) fn new_rand_height(
        key: K,
        val: V,
        list: &SkipList<K, V>,
    ) -> *mut Self {
        // construct the base nod
        Self::new(key, val, list.gen_height())
    }

    pub(super) unsafe fn alloc(height: usize) -> *mut Self {
        let layout = Self::get_layout(height);

        let ptr = alloc(layout).cast::<Self>();

        if ptr.is_null() {
            handle_alloc_error(layout);
        }

        ptr::write(&mut (*ptr).height_and_removed, AtomicUsize::new(height));

        ptr::write_bytes((*ptr).levels.pointers.as_mut_ptr(), 0, height);

        ptr
    }

    pub(super) unsafe fn dealloc(ptr: *mut Self) {
        let height = (*ptr).height();

        let layout = Self::get_layout(height);

        dealloc(ptr.cast(), layout);
    }

    unsafe fn get_layout(height: usize) -> Layout {
        let size_self = mem::size_of::<Self>();
        let align = mem::align_of::<Self>();
        let size_levels = Levels::<K, V>::get_size(height);

        Layout::from_size_align_unchecked(size_self + size_levels, align)
    }

    pub(super) unsafe fn drop(ptr: *mut Self) {
        ptr::drop_in_place(&mut (*ptr).key);
        ptr::drop_in_place(&mut (*ptr).val);

        Node::dealloc(ptr);
    }

    pub(super) fn height(&self) -> usize {
        (self.height_and_removed.load(Ordering::Relaxed) & HEIGHT_MASK) as usize
    }

    pub(super) fn refs(&self) -> usize {
        (self.height_and_removed.load(Ordering::SeqCst) & !REMOVED_MASK)
            >> (HEIGHT_BITS + 1)
    }

    pub(super) fn add_ref(&self) -> usize {
        let refs = self
            .height_and_removed
            .fetch_add(1 << (HEIGHT_BITS + 1), Ordering::SeqCst)
            as usize;

        refs
    }

    pub(super) fn try_add_ref(&self) -> Result<usize, usize> {
        self.height_and_removed
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |o| {
                if (o & !REMOVED_MASK) >> (HEIGHT_BITS + 1) == 0 {
                    return None;
                }

                Some(o + (1 << (HEIGHT_BITS + 1)))
            })
            .map(|now| ((now & !REMOVED_MASK) >> (HEIGHT_BITS + 1)) + 1)
    }

    pub(super) fn sub_ref(&self) -> usize {
        let prev = self
            .height_and_removed
            .fetch_sub(1 << (HEIGHT_BITS + 1), Ordering::SeqCst);
        ((prev & !REMOVED_MASK) >> (HEIGHT_BITS + 1)) - 1
    }

    pub(super) fn removed(&self) -> bool {
        self.height_and_removed.load(Ordering::Acquire).leading_zeros() == 0
    }

    pub(crate) fn set_removed(&self) -> Result<usize, ()> {
        self.set_har_with(|old| old | REMOVED_MASK)
    }

    fn set_har_with<F>(&self, f: F) -> Result<usize, ()>
    where
        F: Fn(usize) -> usize,
    {
        let height_and_removed = self.height_and_removed.load(Ordering::SeqCst);

        let new_height_and_removed = f(height_and_removed);

        if new_height_and_removed == height_and_removed {
            return Err(());
        }

        // try to exchange
        self.height_and_removed
            .compare_exchange(
                height_and_removed,
                new_height_and_removed,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .map_err(|_| ())
    }

    pub(super) fn tag_levels(&self, tag: usize) -> Result<usize, usize> {
        for level in (0 .. self.height()).rev() {
            if let Err(o_tag) = self.levels[level].compare_exchange_tag(
                0,
                tag,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                return Err(o_tag);
            }
        }
        Ok(self.height() - 1)
    }

    pub(crate) fn try_remove_and_tag(&self) -> Result<(), ()> {
        self.set_removed()?;

        self.tag_levels(1).map_err(|_| ())?;

        Ok(())
    }
}

impl<K, V> PartialEq for Node<K, V>
where
    K: PartialEq,
    V: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.val == other.val
    }
}

impl<K, V> Debug for Node<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node")
            .field("key", &self.key)
            .field("val", &self.val)
            .field("height", &self.height())
            .field(
                "levels",
                &(0 .. self.height()).fold(String::new(), |acc, level| {
                    format!("{}{:?}, ", acc, self.levels[level].as_std())
                }),
            )
            .finish()
    }
}

impl<K, V> Display for Node<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (1 ..= self.levels.pointers.len()).try_for_each(|level| {
            writeln!(
                f,
                "[key:  {:?}, val: {:?}, level: {}]",
                self.key, self.val, level,
            )
        })
    }
}

mod node_test {
    use super::*;

    #[test]
    fn test_removed() {
        unsafe {
            let node = Node::new(1, (), 3);

            assert!(!(*node).removed());

            assert!((*node).set_removed().is_ok());

            assert!((*node).removed());

            (*node).add_ref();

            assert_eq!((*node).refs(), 1);

            assert_eq!((*node).try_add_ref().unwrap(), 2);
        }
    }
}
