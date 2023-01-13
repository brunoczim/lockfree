#![no_main]
#[macro_use]
extern crate libfuzzer_sys;
extern crate fuzzsuite;
extern crate lockfree;

use fuzzsuite::*;
use lockfree::prelude::*;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
struct SkipListMachine {
    list: Arc<SkipList<Box<u8>, Box<u8>>>,
}

impl Spawn for SkipListMachine {
    fn spawn() -> Self {
        Self::default()
    }

    fn fork(&self) -> Self {
        self.clone()
    }
}

impl Machine for SkipListMachine {
    fn interpret(&mut self, mut byte: u8, bytecode: &mut Bytecode) {
        loop {
            match byte % 4 {
                0 | 1 => {
                    let val = bytecode.next().unwrap_or(0);
                    byte = self
                        .list
                        .remove(&Box::new(val))
                        .map_or(bytecode.next().unwrap_or(0), |e| *e.val);
                },

                2 | 3 => {
                    let val = bytecode.next().unwrap_or(0);
                    self.list.insert(Box::new(val), Box::new(val));
                    break;
                },
                _ => unreachable!(),
            }
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let _ = test::<SkipListMachine>(Bytecode::no_symbols(data));
});
