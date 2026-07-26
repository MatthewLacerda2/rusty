//! src/render/setup/budget.rs — a bound on how many headless renderers exist at once.
//!
//! A [`Renderer`](crate::render::Renderer) is not a small object: a wgpu device, the
//! full pipeline set, and the shadow maps. Nothing used to stop N of them existing
//! simultaneously, so the test suite's peak GPU memory was whatever the machine
//! happened to schedule — an *emergent* ceiling, written down nowhere.
//!
//! That ceiling was real. On the Windows CI runner there is no GPU, so wgpu selects
//! **WARP**, Microsoft's software rasterizer, whose textures live in ordinary system
//! RAM alongside rustc and the test harness. Adding three GPU tests in #365 pushed it
//! over, and the failure landed on two *unrelated* pre-existing tests as a bare
//! `Queue::write_texture: Not enough memory left.` — a diagnosis that cost a
//! cross-platform timing comparison and a feature-set differential to reach (#366).
//!
//! So the limit becomes explicit: [`MAX_CONCURRENT_HEADLESS`], enforced by an RAII
//! permit that a headless renderer holds for its whole life.
//!
//! **Why this lives in the engine rather than in a test helper.** Integration tests
//! (`tests/*.rs`) link the *normal* library build, not the `cfg(test)` one, and they
//! reach the renderer indirectly — through `screenshot::capture`, `Debug.Preview`,
//! and the probe bakes. A `#[cfg(test)]` guard would miss every one of them. Sitting
//! inside the single constructor they all funnel through, this covers direct and
//! indirect creation alike, with no call site needing to know it exists.
//!
//! The windowed renderer deliberately takes **no** permit: there is exactly one, it is
//! the application itself, and making the app queue behind a test budget would be
//! backwards.

use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// How many headless renderers may be alive at once, process-wide.
///
/// **Two, not one:** one test can build its renderer while another is still tearing
/// its own down, so the common case never blocks, while peak memory stays bounded at
/// two devices instead of one-per-core. Two also leaves headroom for a call path that
/// legitimately holds one renderer while building another — at a cap of one that would
/// be an instant self-deadlock.
///
/// **If WARP runs out of memory again, lower this to 1 before anything else.** That
/// fully serializes headless renderers and makes the suite's GPU memory deterministic;
/// the cost is wall-clock on the GPU tests, which are a small minority of the suite.
/// Raising it is the move that needs justification, not lowering it.
pub(crate) const MAX_CONCURRENT_HEADLESS: usize = 2;

/// How long to wait for a permit before declaring the budget deadlocked.
///
/// Generous on purpose: a cold WARP renderer plus a probe bake is slow, and a false
/// positive here would be a confusing CI failure. This is not a performance budget —
/// it exists so a *nested* acquire (a permit-holder building a second renderer on the
/// same thread, which can never be satisfied) fails with an explanation instead of
/// hanging until the job times out with no output at all.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(180);

/// The process-wide headless-renderer budget.
static HEADLESS: Budget = Budget::new(MAX_CONCURRENT_HEADLESS, ACQUIRE_TIMEOUT);

/// A counting semaphore with a deadlock timeout.
///
/// A plain struct rather than loose statics so the mechanism can be exercised against
/// a private instance: its tests must not share a counter with the real GPU tests
/// running beside them in the same binary, or the two would race over the same numbers.
pub(crate) struct Budget {
    cap: usize,
    timeout: Duration,
    live: Mutex<usize>,
    released: Condvar,
}

impl Budget {
    const fn new(cap: usize, timeout: Duration) -> Self {
        Self {
            cap,
            timeout,
            live: Mutex::new(0),
            released: Condvar::new(),
        }
    }

    /// Take one slot, blocking while all of them are held.
    ///
    /// # Panics
    ///
    /// After `timeout` with no slot freed — which in practice means a deadlock rather
    /// than contention, since permits are released on drop. The message names the
    /// likely cause, because the failure it replaces (an unexplained hang) is the
    /// single worst thing a CI job can do.
    pub(crate) fn acquire(&'static self) -> Permit {
        let mut live = self.live.lock().unwrap_or_else(|e| e.into_inner());
        while *live >= self.cap {
            let (guard, wait) = self
                .released
                .wait_timeout(live, self.timeout)
                .unwrap_or_else(|e| e.into_inner());
            live = guard;
            assert!(
                !wait.timed_out() || *live < self.cap,
                "timed out waiting for a headless-renderer slot ({} of {} still held \
                 after {:?}). Permits are released on drop, so this is almost certainly \
                 a nested acquire: something built a second headless Renderer while \
                 still holding the first. Drop the first, share it, or raise \
                 MAX_CONCURRENT_HEADLESS in src/render/setup/budget.rs.",
                *live,
                self.cap,
                self.timeout,
            );
        }
        *live += 1;
        Permit { budget: self }
    }

    /// Slots currently held. Test-only: nothing in the engine needs to read the
    /// count, it only needs the permits to be correct.
    #[cfg(test)]
    fn live(&self) -> usize {
        *self.live.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Proof that its holder is within the budget. Acquired before the wgpu device is
/// created and released when the renderer is dropped, so the count tracks live GPU
/// memory rather than merely the rate of construction.
pub(crate) struct Permit {
    budget: &'static Budget,
}

impl Drop for Permit {
    fn drop(&mut self) {
        let mut live = self.budget.live.lock().unwrap_or_else(|e| e.into_inner());
        *live = live.saturating_sub(1);
        // One waiter per freed slot; waking all would have them contend for one slot.
        self.budget.released.notify_one();
    }
}

/// Take a slot in the process-wide headless-renderer budget.
pub(crate) fn acquire_headless() -> Permit {
    HEADLESS.acquire()
}

#[cfg(test)]
#[path = "budget_tests.rs"]
mod budget_tests;
