#[macro_use]
extern crate benchsuite;
extern crate lockfree;
extern crate rand;

use benchsuite::exec::Target;
use lockfree::skiplist::SkipList;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

type MutexBTreeMapInner = Arc<Mutex<BTreeMap<u8, u8>>>;

type LockfreeInner = Arc<SkipList<u8, u8>>;

fn randomize(mut i: usize) -> usize {
    i ^= i >> 12;
    i ^= i << 25;
    i ^ i >> 27
}

#[derive(Debug, Clone, Default)]
struct MutexBTreeMapInsert {
    inner: MutexBTreeMapInner,
    seed: usize,
}

impl Target for MutexBTreeMapInsert {
    #[inline(always)]
    fn round(&mut self) {
        let i = randomize(self.seed);

        self.inner.lock().unwrap().insert(i as u8, i as u8);

        self.seed = i;
    }
}

#[derive(Debug, Clone, Default)]
struct LockfreeInsert {
    inner: LockfreeInner,
    seed: usize,
}

impl Target for LockfreeInsert {
    #[inline(always)]
    fn round(&mut self) {
        let i = randomize(self.seed);

        self.inner.insert(i as u8, i as u8);

        self.seed = i;
    }
}

#[derive(Debug, Clone, Default)]
struct MutexBTreeMapGet {
    inner: MutexBTreeMapInner,
    seed: usize,
}

impl Target for MutexBTreeMapGet {
    #[inline(always)]
    fn round(&mut self) {
        let i = randomize(self.seed);

        self.inner.lock().unwrap().get(&(i as u8));

        self.seed = i;
    }
}

#[derive(Debug, Clone, Default)]
struct LockfreeGet {
    inner: LockfreeInner,
    seed: usize,
}

impl Target for LockfreeGet {
    #[inline(always)]
    fn round(&mut self) {
        let i = randomize(self.seed);

        self.inner.get(&(i as u8));

        self.seed = i;
    }
}

#[derive(Debug, Clone, Default)]
struct MutexBTreeMapPopFirst {
    inner: MutexBTreeMapInner,
}

impl Target for MutexBTreeMapPopFirst {
    #[inline(always)]
    fn round(&mut self) {
        self.inner.lock().unwrap().pop_first();
    }
}

#[derive(Debug, Clone, Default)]
struct LockfreePopFirst {
    inner: LockfreeInner,
}

impl Target for LockfreePopFirst {
    #[inline(always)]
    fn round(&mut self) {
        self.inner.pop_first();
    }
}

fn main() {
    let mutex = MutexBTreeMapInner::default();
    let lockfree = LockfreeInner::default();
    bench! {
        levels 1, 2, 4, 8, 16, 32;
        "mutex btree_map insert" => MutexBTreeMapInsert {
            inner: mutex.clone(),
            seed: rand::random::<usize>(),
        },
        "lockfree insert" => LockfreeInsert {
            inner: lockfree.clone(),
            seed: rand::random::<usize>(),
        },
    }

    bench! {
        levels 1, 2, 4, 8, 16, 32;
        "mutex btree_map get" => MutexBTreeMapGet {
            inner: mutex.clone(),
            seed: rand::random::<usize>(),
        },
        "lockfree get" => LockfreeGet {
            inner: lockfree.clone(),
            seed: rand::random::<usize>(),
        },
    }

    bench! {
        levels 1, 2, 4, 8, 16, 32;
        "mutex btree_map pop_first" => MutexBTreeMapPopFirst {
            inner: mutex.clone(),
        },
        "lockfree get pop_first" => LockfreePopFirst {
            inner: lockfree.clone(),
        },
    }
}
