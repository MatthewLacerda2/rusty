//! Bounds when a large chart's denoise starts, never what it gives, so the bake's bytes do not
//! depend on it.

use std::sync::{Condvar, Mutex, PoisonError};

use super::ffi::{KEEP_TEXELS, SCRATCH_MB};

/// Bytes the large denoises in flight may hold together; one alone always starts, however large.
pub const BUDGET: u64 = 2 << 30;

/// Bytes per texel a denoise holds besides its filter's scratch; `memory_tests` holds a chart
/// to it.
pub const PER_TEXEL: u64 = 256;

/// Bytes of scratch per texel an untiled filter takes, until `SCRATCH_MB` tiles it.
const SCRATCH_PER_TEXEL: u64 = 1200;

pub fn cost(texels: usize) -> u64 {
    let texels = texels as u64;
    let scratch = (texels * SCRATCH_PER_TEXEL).min(SCRATCH_MB as u64 * (1 << 20));
    texels * PER_TEXEL + scratch
}

#[derive(Default)]
pub struct Budget {
    held: Mutex<(u64, usize)>,
    freed: Condvar,
}

impl Budget {
    pub fn run<T>(&self, texels: usize, work: impl FnOnce() -> T) -> T {
        if texels <= KEEP_TEXELS {
            return work();
        }
        let bytes = cost(texels);
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        while held.1 > 0 && held.0 + bytes > BUDGET {
            held = self
                .freed
                .wait(held)
                .unwrap_or_else(PoisonError::into_inner);
        }
        *held = (held.0 + bytes, held.1 + 1);
        drop(held);
        let _back = Back {
            budget: self,
            bytes,
        };
        work()
    }
}

struct Back<'a> {
    budget: &'a Budget,
    bytes: u64,
}

impl Drop for Back<'_> {
    fn drop(&mut self) {
        let mut held = self
            .budget
            .held
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *held = (held.0 - self.bytes, held.1 - 1);
        self.budget.freed.notify_all();
    }
}
