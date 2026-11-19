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
use std::sync::{Mutex, OnceLock, PoisonError};
use std::thread;

use rusty_core::quiet::{holding, say};

/// The work, in triangles, below which a call stays on the caller's thread:
/// spawning costs more than it saves.
pub const SMALL: usize = 4096;

thread_local! {
    /// The thread count [`with_threads`] pinned on this thread, if any.
    static PINNED: Cell<Option<usize>> = const { Cell::new(None) };
}

/// Run `f` with every [`map`] on this thread using exactly `threads` threads (at
/// least 1), whatever the work's size: a determinism probe, the output must not
/// change with it.
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

/// The machine's cores, asked once.
fn cores() -> usize {
    static CORES: OnceLock<usize> = OnceLock::new();
    *CORES.get_or_init(|| thread::available_parallelism().map_or(1, |n| n.get()))
}

/// How many threads `items` items of `work` triangles in all are spread over.
fn threads_for(items: usize, work: usize) -> usize {
    let n = match PINNED.with(Cell::get) {
        Some(pinned) => pinned,
        None if work < SMALL => 1,
        None => cores(),
    };
    n.clamp(1, items.max(1))
}

/// `f` of every item, in the items' order, the calls spread over threads; `size`
/// is an item's work in triangles. Little work, one item or one thread runs on
/// the caller's thread. A warning an item raises through `rusty_core::warn_held!`
/// is said by the caller after the join, in item order, as one thread says it
/// (`core::quiet::holding`).
pub fn map<T: Send, R: Send>(
    items: Vec<T>,
    size: impl Fn(&T) -> usize,
    f: impl Fn(T) -> R + Sync,
) -> Vec<R> {
    let work = items.iter().map(size).sum();
    let threads = threads_for(items.len(), work);
    if threads == 1 {
        return items.into_iter().map(f).collect();
    }
    let lock = |m: &Mutex<Option<T>>| m.lock().unwrap_or_else(PoisonError::into_inner).take();
    let inputs: Vec<Mutex<Option<T>>> = items.into_iter().map(|t| Mutex::new(Some(t))).collect();
    let outputs: Vec<Mutex<Option<_>>> = inputs.iter().map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    let work = || loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        let Some(slot) = inputs.get(i) else { return };
        if let Some(item) = lock(slot) {
            let out = holding(|| f(item));
            *outputs[i].lock().unwrap_or_else(PoisonError::into_inner) = Some(out);
        }
    };
    thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(work);
        }
    });
    let taken = outputs.into_iter().map(|m| {
        let (out, held) = m
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
            .expect("every item is taken by exactly one thread, which fills its slot");
        say(held);
        out
    });
    taken.collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_results_keep_the_items_order_whatever_the_thread_count() {
        let items: Vec<u64> = (0..97).collect();
        let square = |x: u64| x * x;
        let serial = with_threads(1, || map(items.clone(), |_| 1, square));
        for threads in [2, 3, 8, 200] {
            assert_eq!(
                with_threads(threads, || map(items.clone(), |_| 1, square)),
                serial
            );
        }
        assert_eq!(serial[96], 96 * 96);
        assert!(map(Vec::<u64>::new(), |_| 1, square).is_empty());
    }

    #[test]
    fn a_workers_warnings_are_said_by_the_caller_in_item_order() {
        rusty_test_support::logs::init();
        rusty_test_support::logs::take();
        let items: Vec<u32> = (0..12).collect();
        let warn = |x: u32| rusty_core::warn_held!("item {x}");
        with_threads(4, || map(items, |_| 1, warn));
        let want: Vec<String> = (0..12).map(|x| format!("item {x}")).collect();
        assert_eq!(rusty_test_support::logs::take(), want);
    }

    #[test]
    fn little_work_stays_on_the_callers_thread() {
        assert_eq!(threads_for(8, SMALL - 1), 1);
        assert_eq!(threads_for(1, SMALL * 10), 1);
        assert_eq!(threads_for(2, SMALL), cores().min(2));
    }

    #[test]
    fn a_pin_is_undone_after_its_call() {
        with_threads(3, || assert_eq!(threads_for(10, 0), 3));
        assert_eq!(PINNED.with(Cell::get), None);
        assert_eq!(with_threads(0, || threads_for(10, 0)), 1);
        assert_eq!(with_threads(9, || threads_for(2, 0)), 2);
    }
}
