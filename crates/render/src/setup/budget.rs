use std::sync::{Condvar, Mutex};
use std::thread::ThreadId;
use std::time::Duration;

#[cfg(target_os = "macos")]
pub(crate) const MAX_CONCURRENT_HEADLESS: usize = 3;
#[cfg(not(target_os = "macos"))]
pub(crate) const MAX_CONCURRENT_HEADLESS: usize = 2;

const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(180);

static HEADLESS: Budget = Budget::new(MAX_CONCURRENT_HEADLESS, ACQUIRE_TIMEOUT, test_thread);

fn test_thread() -> Option<ThreadId> {
    let thread = std::thread::current();
    thread.name()?.contains("::").then(|| thread.id())
}

pub(crate) struct Budget {
    cap: usize,
    timeout: Duration,
    holder: fn() -> Option<ThreadId>,
    held: Mutex<Held>,
    released: Condvar,
}

struct Held {
    live: usize,
    holders: Vec<Holder>,
}

struct Holder {
    thread: ThreadId,
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
                 (a Pair), or raise MAX_CONCURRENT_HEADLESS in crates/render/src/setup/budget.rs.",
                held.live,
                self.cap,
                self.timeout,
            );
        }
        held.live += slots;
        held.holders.extend(holder.map(|thread| Holder {
            thread,
            pair: std::sync::Weak::new(),
        }));
        holder
    }

    fn give(&self, slots: usize, holder: Option<ThreadId>) {
        let mut held = self.lock();
        held.live = held.live.saturating_sub(slots);
        if let Some(at) = holder.and_then(|id| held.holders.iter().position(|h| h.thread == id)) {
            held.holders.swap_remove(at);
        }
        self.released.notify_all();
    }

    pub(crate) fn acquire(&'static self) -> Permit {
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

pub(crate) struct Permit(Grant);

enum Grant {
    Slot {
        budget: &'static Budget,
        holder: Option<ThreadId>,
    },
    InPair(
        #[expect(dead_code, reason = "held for its drop, which gives the share back")] pair::Share,
    ),
}

impl Drop for Permit {
    fn drop(&mut self) {
        match &self.0 {
            Grant::Slot { budget, holder } => budget.give(1, *holder),
            Grant::InPair(_) => {}
        }
    }
}

pub(crate) fn acquire_headless() -> Permit {
    super::gpu_rule::check_current_thread();
    HEADLESS.acquire()
}

mod pair;
pub(crate) use pair::{acquire_headless_pair, Pair};

#[cfg(test)]
mod holder_tests;
#[cfg(test)]
mod tests;
