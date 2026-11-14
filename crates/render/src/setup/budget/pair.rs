//! Two headless slots taken together (#1844): what a test keeping two renderers
//! alive at once holds, so it never holds one slot while waiting for the other.
//!
//! Only tests take one (it is compiled in for the crates above, whose tests link
//! the normal build): no engine path holds two headless renderers on purpose. A test
//! reaches a pair through `render::test_gpu::headless_pair_or_skip`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::thread::ThreadId;

use super::{Budget, Grant, Holder, Permit, HEADLESS};

/// Two slots of a [`Budget`]. Each [`Pair::permit`] is one renderer's share; the
/// slots go back when the pair and every share are gone.
pub(crate) struct Pair(Arc<PairSlots>);

pub(super) struct PairSlots {
    budget: &'static Budget,
    holder: Option<ThreadId>,
    /// Renderers alive on this pair's slots.
    renderers: AtomicUsize,
}

/// One renderer's share of a pair, held in its [`Permit`].
pub(super) struct Share(Arc<PairSlots>);

impl Budget {
    /// Take two slots at once (panics: [`Budget::take`]). A checked thread's pair
    /// then serves every single acquire on that thread ([`Budget::acquire`]).
    pub(crate) fn acquire_pair(&'static self) -> Pair {
        let holder = self.take(2);
        let slots = Arc::new(PairSlots {
            budget: self,
            holder,
            renderers: AtomicUsize::new(0),
        });
        if let Some(id) = holder {
            let mut held = self.lock();
            let entry = held.holders.iter_mut().find(|h| h.thread == id);
            entry.expect("take recorded it").pair = Arc::downgrade(&slots);
        }
        Pair(slots)
    }

    /// The pair the calling checked thread holds, if it holds one.
    pub(super) fn pair_of_this_thread(&self) -> Option<Pair> {
        let id = (self.holder)()?;
        let held = self.lock();
        let holder: &Holder = held.holders.iter().find(|h| h.thread == id)?;
        Weak::upgrade(&holder.pair).map(Pair)
    }
}

impl Pair {
    /// One renderer's share of the pair.
    ///
    /// # Panics
    ///
    /// When two renderers already live on it: a pair holds two slots, not more.
    pub(crate) fn permit(&self) -> Permit {
        let alive = self.0.renderers.fetch_add(1, Ordering::SeqCst) + 1;
        if alive > 2 {
            self.0.renderers.fetch_sub(1, Ordering::SeqCst);
            panic!("a third headless renderer on a pair, which holds two slots");
        }
        Permit(Grant::InPair(Share(Arc::clone(&self.0))))
    }
}

impl Drop for Share {
    fn drop(&mut self) {
        self.0.renderers.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Drop for PairSlots {
    fn drop(&mut self) {
        self.budget.give(2, self.holder);
    }
}

/// Take two slots in the process-wide budget at once.
pub(crate) fn acquire_headless_pair() -> Pair {
    super::super::gpu_rule::check_current_thread();
    HEADLESS.acquire_pair()
}
