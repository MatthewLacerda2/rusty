//! How much the bake's denoising may hold at once (#2312). Each worker cleans its
//! own charts on a device of its own, so with no bound the denoise's memory grew
//! with the cores: every worker finishing a large chart at the same time held one
//! large chart's filter. [`Budget`] lets a chart's denoise start only while what the
//! ones in flight are [`cost`] stays within [`BUDGET`]; one alone always starts,
//! so a chart larger than the budget still runs, by itself. Small charts ([`cost`]
//! of a kept filter's size) run beside anything. Only when a denoise runs moves,
//! never what it gives, so the bake's bytes do not depend on it.

use std::sync::{Condvar, Mutex, PoisonError};

use super::ffi::{KEEP_TEXELS, SCRATCH_MB};

/// The bytes the denoises in flight may hold together.
pub const BUDGET: u64 = 2 << 30;

/// Bytes a texel costs a denoise beside its filter's scratch: the colour and
/// output images handed to OIDN, its own copies, and the chart's padded image
/// (measured in #2312's PR at about 200).
pub const PER_TEXEL: u64 = 256;

/// Bytes of scratch an untiled filter takes per texel, up to [`SCRATCH_MB`].
const SCRATCH_PER_TEXEL: u64 = 1200;

/// What denoising one `texels`-texel image holds while it runs.
pub fn cost(texels: usize) -> u64 {
    let texels = texels as u64;
    let scratch = (texels * SCRATCH_PER_TEXEL).min(SCRATCH_MB as u64 * (1 << 20));
    texels * PER_TEXEL + scratch
}

/// The denoises in flight, and what they hold.
#[derive(Default)]
pub struct Budget {
    held: Mutex<(u64, usize)>,
    freed: Condvar,
}

impl Budget {
    /// Run `work`, a denoise of `texels` texels, once the budget has room for it.
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
        // Given back even when `work` panics, so no waiting worker is left behind.
        let _back = Back {
            budget: self,
            bytes,
        };
        work()
    }
}

/// What one denoise in flight holds, given back when it ends.
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
