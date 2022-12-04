use std::{
    cmp::Ordering::{Equal, Greater, Less},
    fmt::Debug,
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
    vec,
};

/// A lock-free binary search tree that that currently only supports concurrent
/// pushing with removal for now only working through a mutable reference.
pub struct LockFreeBST<K, V> {
    head: AtomicPtr<TreeNode<K, V>>,
}

impl<K: Ord, V> LockFreeBST<K, V> {
    /// Creates a new empty binary search tree.
    pub fn new() -> LockFreeBST<K, V> {
        LockFreeBST { head: AtomicPtr::new(ptr::null_mut()) }
    }

    /// Inserts a new key-value pair into the tree. If a value with the same key
    /// already exists it returns the old key-value pair.
    pub fn insert(&self, key: K, value: V) -> Option<(K, V)> {
        let mut new_node = Box::into_raw(Box::new(TreeNode::new(key, value)));
        let mut curr = &self.head;
        let mut curr_ptr = self.head.load(Ordering::Acquire);
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
                            curr = &current_ref.left;
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
                            curr = &current_ref.right;
                            curr_ptr = right;
                        }
                    },
                    Equal => {
                        let TreeNode { right, left, key, value } =
                            unsafe { ptr::read(current_ref) };

                        unsafe {
                            (*new_node).left = left;
                            (*new_node).right = right;
                        };

                        match curr.compare_exchange(
                            curr_ptr,
                            new_node,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        ) {
                            Ok(_) => return Some((key, value)),
                            Err(new_ptr) => curr_ptr = new_ptr,
                        }
                    },
                }
            }
        }
    }

    /// Traverses the tree in sorted order and returns an iterator of owned
    /// values.
    pub fn order_traversal(&self) -> impl Iterator<Item = (K, V)>
    where
        K: Clone,
        V: Clone,
    {
        fn traverse_and_collect<K, V>(
            node: *mut TreeNode<K, V>,
            res: &mut Vec<(K, V)>,
        ) where
            K: Clone,
            V: Clone,
        {
            if !node.is_null() {
                let node = unsafe { &*node };
                traverse_and_collect(node.left.load(Ordering::SeqCst), res);
                res.push((node.key.clone(), node.value.clone()));
                traverse_and_collect(node.right.load(Ordering::SeqCst), res);
            }
        }

        let curr_ptr = self.head.load(Ordering::Relaxed);
        let mut kvps = vec![];

        traverse_and_collect(curr_ptr, &mut kvps);

        kvps.into_iter()
    }

    /// Drains all elements of the tree and returns them sorted in an iterator.
    pub fn drain(&mut self) -> impl Iterator<Item = (K, V)> {
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
                            kvps.push((key, value));
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
}

impl<K, V> Debug for LockFreeBST<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let head = self.head.load(Ordering::Relaxed);
        if !head.is_null() {
            unsafe { write!(f, "LockFreeBST {{ {:?} }}", *head) }
        } else {
            write!(f, "LockFreeBST {{}}")
        }
    }
}

impl<K, V> Drop for LockFreeBST<K, V> {
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

#[repr(align(2))]
struct TreeNode<K, V> {
    key: K,
    value: V,
    left: AtomicPtr<TreeNode<K, V>>,
    right: AtomicPtr<TreeNode<K, V>>,
}

impl<K: Ord, V> TreeNode<K, V> {
    pub fn new(key: K, value: V) -> TreeNode<K, V> {
        TreeNode {
            key,
            value,
            left: AtomicPtr::new(ptr::null_mut()),
            right: AtomicPtr::new(ptr::null_mut()),
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
        let tree = LockFreeBST::<i32, String>::new();
        tree.insert(1, "ffs".into());
        tree.insert(4, "203".into());

        tree.insert(3, "hello".into());
        tree.insert(3, "there".into());
        tree.insert(1, "yess".into());

        println!("{:?}", tree);

        println!(
            "{:?}",
            tree.order_traversal().collect::<Vec<(i32, String)>>()
        );
    }

    struct CountOnDrop {
        counter: *const AtomicUsize,
    }

    impl CountOnDrop {
        fn from_ref(counter: &AtomicUsize) -> Self {
            Self { counter }
        }
    }

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
        let tree = LockFreeBST::<i32, CountOnDrop>::new();
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
        let mut tree = LockFreeBST::<i32, i32>::new();
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
}
