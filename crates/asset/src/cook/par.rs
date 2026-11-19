//! crates/asset/src/cook/par.rs — a model's per-sub-mesh work spread over threads,
//! the results kept in the order of the items (#2332).
//!
//! Each item's work is a pure function of that item: nothing is shared between
//! them, and each result lands in its own item's slot, so the output is the same
//! value, bit for bit, whatever the thread count or which thread ran which item.
//! That is why the count may follow the machine's cores
//! ([`std::thread::available_parallelism`]) inside a sim module; [`with_threads`]
//! pins it, so a test can prove the output does not move with it. Threads take
//! the next item as they finish one, so one big sub-mesh does not hold up a
//! thread's share of small ones.

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;

thread_local! {
    /// The thread count [`with_threads`] pinned on this thread, if any.
    static PINNED: Cell<Option<usize>> = const { Cell::new(None) };
}

/// Run `f` with every [`each_mut`] on this thread using exactly `threads` threads
/// (at least 1): a determinism probe, the output must not change with it.
pub fn with_threads<R>(threads: usize, f: impl FnOnce() -> R) -> R {
    /// Restores the previous pin even if `f` panics.
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            PINNED.with(|p| p.set(self.0));
        }
    }
    let _restore = Restore(PINNED.with(|p| p.replace(Some(threads.max(1)))));
    f()
}

/// How many threads `items` items are spread over.
fn threads_for(items: usize) -> usize {
    let n = PINNED
        .with(Cell::get)
        .unwrap_or_else(|| thread::available_parallelism().map_or(1, |n| n.get()));
    n.clamp(1, items.max(1))
}

/// `f` of every item, in the items' order, the calls spread over threads. One item,
/// or one thread, runs on the caller's thread.
pub fn map<T: Send, R: Send>(items: Vec<T>, f: impl Fn(T) -> R + Sync) -> Vec<R> {
    let threads = threads_for(items.len());
    if threads == 1 {
        return items.into_iter().map(f).collect();
    }
    let lock = |m: &Mutex<Option<T>>| m.lock().unwrap_or_else(PoisonError::into_inner).take();
    let inputs: Vec<Mutex<Option<T>>> = items.into_iter().map(|t| Mutex::new(Some(t))).collect();
    let outputs: Vec<Mutex<Option<R>>> = inputs.iter().map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    let work = || loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        let Some(slot) = inputs.get(i) else { return };
        if let Some(item) = lock(slot) {
            let out = f(item);
            *outputs[i].lock().unwrap_or_else(PoisonError::into_inner) = Some(out);
        }
    };
    thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(work);
        }
    });
    let taken = outputs.into_iter().map(|m| {
        m.into_inner()
            .unwrap_or_else(PoisonError::into_inner)
            .expect("every item is taken by exactly one thread, which fills its slot")
    });
    taken.collect()
}

/// Call `f` on every item in place, the calls spread over threads as [`map`]'s.
pub fn each_mut<T: Send>(items: &mut [T], f: impl Fn(&mut T) + Sync) {
    map(items.iter_mut().collect(), f);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_results_keep_the_items_order_whatever_the_thread_count() {
        let items: Vec<u64> = (0..97).collect();
        let square = |x: u64| x * x;
        let serial = with_threads(1, || map(items.clone(), square));
        for threads in [2, 3, 8, 200] {
            assert_eq!(with_threads(threads, || map(items.clone(), square)), serial);
        }
        assert_eq!(serial[96], 96 * 96);
        assert!(map(Vec::<u64>::new(), square).is_empty());
    }

    #[test]
    fn each_mut_reaches_every_item_once() {
        let mut items = vec![1u32; 40];
        with_threads(4, || each_mut(&mut items, |x| *x += 1));
        assert_eq!(items, vec![2u32; 40]);
    }

    #[test]
    fn a_pin_is_undone_after_its_call() {
        with_threads(3, || assert_eq!(threads_for(10), 3));
        assert_eq!(PINNED.with(Cell::get), None);
        assert_eq!(with_threads(0, || threads_for(10)), 1);
        assert_eq!(with_threads(9, || threads_for(2)), 2);
    }
}
