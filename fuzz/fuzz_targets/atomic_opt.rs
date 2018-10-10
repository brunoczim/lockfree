#![no_main]
#[macro_use]
extern crate libfuzzer_sys;
extern crate fuzzsuite;
extern crate lockfree;

use fuzzsuite::*;
use lockfree::atomic::{
    Atomic,
    AtomicOptionBox,
    Ordering::{self, *},
};
use std::sync::Arc;

#[derive(Debug, Default, Clone)]
struct BoxMachine {
    ptr: Arc<AtomicOptionBox<(u8, u8)>>,
    cache: Option<(u8, u8)>,
}

fn get_read_ord(byte: u8) -> Ordering {
    match byte % 3 {
        0 => Relaxed,
        1 => SeqCst,
        2 => Acquire,
        _ => unreachable!(),
    }
}

fn get_write_ord(byte: u8) -> Ordering {
    match byte % 3 {
        0 => Relaxed,
        1 => SeqCst,
        2 => Release,
        _ => unreachable!(),
    }
}

fn get_rw_ord(byte: u8) -> Ordering {
    match byte % 5 {
        0 => Relaxed,
        1 => SeqCst,
        2 => Acquire,
        3 => Release,
        4 => AcqRel,
        _ => unreachable!(),
    }
}

impl Machine for BoxMachine {
    fn spawn() -> Self {
        Self::default()
    }

    fn fork(&self) -> Self {
        self.clone()
    }

    #[allow(unused_must_use)]
    fn interpret(&mut self, byte: u8, bytecode: &mut Bytecode) {
        match byte % 10 {
            0 => {
                let ord = get_read_ord(bytecode.next().unwrap_or(0));
                self.cache = self.ptr.load(ord);
            },

            1 => {
                let ord = get_write_ord(bytecode.next().unwrap_or(0));
                self.ptr.store(self.cache, ord);
            },

            2 => {
                let load_ord = get_read_ord(bytecode.next().unwrap_or(0));
                let cas_ord = get_rw_ord(bytecode.next().unwrap_or(0));
                self.ptr.load_cas_loop(
                    |shared| {
                        let local = self.cache;
                        let (x, y) = shared.unwrap_or((0, 0));
                        let res = local.map(|(v, w)| {
                            (v.wrapping_add(x), w.wrapping_add(y))
                        });
                        Some(res)
                    },
                    load_ord,
                    cas_ord,
                );
            },

            3 | 4 => {
                let x = bytecode.next().unwrap_or(0);
                let y = bytecode.next().unwrap_or(0);
                self.cache = Some((x, y));
            },

            7 => self.cache = None,

            5 => {
                let load_ord = get_read_ord(bytecode.next().unwrap_or(0));
                let init = self.ptr.load(load_ord);
                let cas_ord = get_rw_ord(bytecode.next().unwrap_or(0));
                self.ptr.cas_loop(
                    init,
                    |shared| {
                        let local = self.cache;
                        let (x, y) = shared.unwrap_or((0, 0));
                        let res = local.map(|(v, w)| {
                            (v.wrapping_add(x), w.wrapping_add(y))
                        });
                        Some(res)
                    },
                    cas_ord,
                );
            },

            6 => {
                let load_ord = get_read_ord(bytecode.next().unwrap_or(0));
                let init = self.ptr.load(load_ord);
                let cas_ord = get_rw_ord(bytecode.next().unwrap_or(0));
                self.ptr.compare_and_swap(init, self.cache, cas_ord);
            },

            8 => {
                let load_ord = get_read_ord(bytecode.next().unwrap_or(0));
                let init = self.ptr.load(load_ord);
                let succ_ord = get_rw_ord(bytecode.next().unwrap_or(0));
                self.ptr.compare_exchange(init, self.cache, succ_ord, Relaxed);
            },

            9 => {
                let load_ord = get_read_ord(bytecode.next().unwrap_or(0));
                let init = self.ptr.load(load_ord);
                let succ_ord = get_rw_ord(bytecode.next().unwrap_or(0));
                self.ptr
                    .compare_exchange_weak(init, self.cache, succ_ord, Relaxed);
            },

            _ => unreachable!(),
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let _ = test::<BoxMachine>(Bytecode::no_symbols(data));
});
