use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::thread::ThreadId;

use super::{Budget, Grant, Holder, Permit, HEADLESS};

pub(crate) struct Pair(Arc<PairSlots>);

pub(super) struct PairSlots {
    budget: &'static Budget,
    holder: Option<ThreadId>,
    renderers: AtomicUsize,
}

pub(super) struct Share(Arc<PairSlots>);

impl Budget {
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

pub(crate) fn acquire_headless_pair() -> Pair {
    super::super::gpu_rule::check_current_thread();
    HEADLESS.acquire_pair()
}
