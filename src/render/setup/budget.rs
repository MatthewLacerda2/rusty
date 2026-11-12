//! src/render/setup/budget.rs — a bound on how many headless renderers exist at once.
//!
//! A [`Renderer`](crate::render::Renderer) is not a small object: a wgpu device, the
//! full pipeline set, and the shadow maps. Nothing used to stop N of them existing
//! simultaneously, so the test suite's peak GPU memory was whatever the machine
//! happened to schedule — an *emergent* ceiling, written down nowhere.
//!
//! That ceiling was real. A CI runner with no GPU renders on a software adapter, whose
//! textures live in ordinary system RAM alongside rustc and the test harness. Adding
//! three GPU tests in #365 pushed it over, and the failure landed on two *unrelated*
//! pre-existing tests as a bare `Queue::write_texture: Not enough memory left.` — a
//! diagnosis that cost a cross-platform timing comparison and a feature-set
//! differential to reach (#366).
//!
//! So the limit becomes explicit: [`MAX_CONCURRENT_HEADLESS`], enforced by an RAII
//! permit that a headless renderer holds for its whole life.
//!
//! **A test holds one grant at a time** (#1844): one [`Permit`], or one pair (the
//! test-only `budget::pair`) when it keeps two renderers alive together; while it
//! holds the pair, every renderer its thread builds, in the engine too (a bake beside
//! a capture host), takes one of the pair's two slots. Under nextest each test is its
//! own process and gets the whole budget, but plain `cargo test` (the nightly
//! runner-independence job) runs tests on threads of one process, where two tests
//! each holding one slot and waiting for a second deadlock. So a second grant on a
//! test thread that holds one panics at once and names the pair helper.
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
use std::thread::ThreadId;
use std::time::Duration;

/// How many headless renderers may be alive at once, process-wide.
///
/// **Two, not one:** one test can build its renderer while another is still tearing
/// its own down, so the common case never blocks, while peak memory stays bounded at
/// two devices instead of one-per-core. Two is also what a test comparing two
/// renderers takes, as one pair (#1844); at a cap of one it can never be granted.
///
/// **If WARP runs out of memory again, lower this to 1 before anything else.** That
/// fully serializes headless renderers and makes the suite's GPU memory deterministic.
/// The cost is real: the GPU tests are under a tenth of the suite by count, but under
/// nextest this cap (the `gpu` group's `max-threads`) makes them its critical path, so
/// their time ÷ the cap *is* the suite's wall time (#985), and lowering it to 1
/// stretches the run by as much as those tests then take one at a time. The tests
/// that take a pair would then have to go too.
///
/// Raising it is the move that needs justification, and was measured not to have one:
/// on a 4-core lavapipe container, 4 instead of 2 won ~6%, within run-to-run noise,
/// and 3 measured slower than either, because the whole suite is then near its core
/// bound (#985). What does move the wall is a cheaper renderer per test: pipelines a
/// frame may not draw are built on first use (`gpu::pipelines::LazyPipeline`).
pub(crate) const MAX_CONCURRENT_HEADLESS: usize = 2;

/// How long to wait for a permit before declaring the budget deadlocked.
///
/// Generous on purpose: a cold WARP renderer plus a probe bake is slow, and a false
/// positive here would be a confusing CI failure. This is not a performance budget —
/// it exists so a deadlock the holder check can't see (a nested acquire on a thread
/// that is not a test's) fails with an explanation instead of hanging until the job
/// times out with no output at all.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(180);

/// The process-wide headless-renderer budget.
static HEADLESS: Budget = Budget::new(MAX_CONCURRENT_HEADLESS, ACQUIRE_TIMEOUT, test_thread);

/// The current thread, when it runs a test: libtest names each test's thread after
/// the test, and every test in this crate sits in a module, so the name has a `::`
/// (the recognition [`gpu_rule`](super::gpu_rule) uses too). No engine thread is
/// named that way.
fn test_thread() -> Option<ThreadId> {
    let thread = std::thread::current();
    thread.name()?.contains("::").then(|| thread.id())
}

/// A counting semaphore with a deadlock timeout and a nested-acquire check.
///
/// A plain struct rather than loose statics so the mechanism can be exercised against
/// a private instance: its tests must not share a counter with the real GPU tests
/// running beside them in the same binary, or the two would race over the same numbers.
pub(crate) struct Budget {
    cap: usize,
    timeout: Duration,
    /// Who an acquire is checked for: `Some` names a thread that may hold only one
    /// grant at a time (a test's), `None` takes no part in the check.
    holder: fn() -> Option<ThreadId>,
    held: Mutex<Held>,
    released: Condvar,
}

/// The budget's state, under one lock.
struct Held {
    /// Slots taken.
    live: usize,
    /// One entry per grant held by a checked thread: a single [`Permit`] or a
    /// pair, each recorded under the thread that took it and removed by that
    /// grant's own drop, on whatever thread it runs, so a renderer dropped
    /// elsewhere leaves the record exact.
    holders: Vec<Holder>,
}

/// A checked thread's grant.
struct Holder {
    thread: ThreadId,
    /// The pair it holds, if the grant is one: its thread's renderers take its
    /// shares (`pair::Pair`).
    #[cfg(test)]
    pair: std::sync::Weak<pair::PairSlots>,
}

impl Budget {
    const fn new(cap: usize, timeout: Duration, holder: fn() -> Option<ThreadId>) -> Self {
        Self {
            cap,
            timeout,
            holder,
            held: Mutex::new(Held {
                live: 0,
                holders: Vec::new(),
            }),
            released: Condvar::new(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Held> {
        self.held.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Take `slots` at once, blocking while fewer are free, never holding some while
    /// waiting for the rest. Returns the checked holder it recorded.
    ///
    /// # Panics
    ///
    /// At once, when the calling test thread already holds a grant: under plain
    /// `cargo test`, where tests share one budget, two tests each holding one slot
    /// and waiting for a second never finish (#1844). And after `timeout` with no
    /// slot freed, which on any other thread means a deadlock rather than
    /// contention, since permits are released on drop.
    fn take(&'static self, slots: usize) -> Option<ThreadId> {
        assert!(
            slots <= self.cap,
            "{slots} headless slots asked of a cap of {}",
            self.cap
        );
        let holder = (self.holder)();
        let mut held = self.lock();
        if let Some(id) = holder {
            assert!(
                !held.holders.iter().any(|h| h.thread == id),
                "this test builds a second headless Renderer while it still holds the \
                 first. Under plain `cargo test` (the nightly cargo-test run) tests share the budget, so two such tests each hold one \
                 slot and wait forever for a second (#1844). A test that needs two \
                 renderers alive at once takes both slots together: \
                 render::test_gpu::headless_pair_or_skip. An integration test \
                 (tests/gpu/), where that helper is out of reach, drops the first \
                 renderer before building the second."
            );
        }
        while held.live + slots > self.cap {
            let (guard, wait) = self
                .released
                .wait_timeout(held, self.timeout)
                .unwrap_or_else(|e| e.into_inner());
            held = guard;
            assert!(
                !wait.timed_out() || held.live + slots <= self.cap,
                "timed out waiting for a headless-renderer slot ({} of {} still held \
                 after {:?}). Permits are released on drop, so this is almost certainly \
                 a nested acquire: something built a second headless Renderer while \
                 still holding the first. Drop the first, share it, take both at once \
                 (a Pair), or raise MAX_CONCURRENT_HEADLESS in src/render/setup/budget.rs.",
                held.live,
                self.cap,
                self.timeout,
            );
        }
        held.live += slots;
        held.holders.extend(holder.map(|thread| Holder {
            thread,
            #[cfg(test)]
            pair: std::sync::Weak::new(),
        }));
        holder
    }

    /// Give back `slots` and the holder record `holder` took them under.
    fn give(&self, slots: usize, holder: Option<ThreadId>) {
        let mut held = self.lock();
        held.live = held.live.saturating_sub(slots);
        if let Some(at) = holder.and_then(|id| held.holders.iter().position(|h| h.thread == id)) {
            held.holders.swap_remove(at);
        }
        // A pair may be waiting for two, so a single freed slot wakes every waiter.
        self.released.notify_all();
    }

    /// Take one slot, blocking while all of them are held (panics: [`Self::take`]).
    pub(crate) fn acquire(&'static self) -> Permit {
        #[cfg(test)]
        if let Some(pair) = self.pair_of_this_thread() {
            return pair.permit();
        }
        let holder = self.take(1);
        Permit(Grant::Slot {
            budget: self,
            holder,
        })
    }

    /// Slots currently held. Test-only: nothing in the engine needs to read the
    /// count, it only needs the permits to be correct.
    #[cfg(test)]
    pub(super) fn live(&self) -> usize {
        self.lock().live
    }
}

/// Proof that its holder is within the budget. Acquired before the wgpu device is
/// created and released when the renderer is dropped, so the count tracks live GPU
/// memory rather than merely the rate of construction.
pub(crate) struct Permit(Grant);

enum Grant {
    /// A slot of its own.
    Slot {
        budget: &'static Budget,
        holder: Option<ThreadId>,
    },
    /// One of a pair's two slots (`pair::Pair`).
    #[cfg(test)]
    InPair(
        #[expect(dead_code, reason = "held for its drop, which gives the share back")] pair::Share,
    ),
}

impl Drop for Permit {
    fn drop(&mut self) {
        match &self.0 {
            Grant::Slot { budget, holder } => budget.give(1, *holder),
            #[cfg(test)]
            Grant::InPair(_) => {}
        }
    }
}

/// Take a slot in the process-wide headless-renderer budget.
pub(crate) fn acquire_headless() -> Permit {
    super::gpu_rule::check_current_thread();
    HEADLESS.acquire()
}

#[cfg(test)]
mod pair;
#[cfg(test)]
pub(crate) use pair::{acquire_headless_pair, Pair};

#[cfg(test)]
mod holder_tests;
#[cfg(test)]
mod tests;
