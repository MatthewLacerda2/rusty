//! A model's per-sub-mesh work spread over threads, results kept in item order.
//!
//! Each item's work is a pure function of that item and lands in its own slot, so the
//! output is bit-identical whatever the thread count; that is why the count may follow
//! the machine's cores inside a sim module. [`with_threads`] pins it for tests.

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::thread;

use rusty_core::quiet::{holding, say};

pub const SMALL: usize = 4096;

/// Cores left free: the main thread's and one for the audio mixer and GPU driver.
pub const KEPT_FOR_PLAY: usize = 2;

thread_local! {
    static PINNED: Cell<Option<usize>> = const { Cell::new(None) };
    static CAP: Cell<Option<usize>> = const { Cell::new(None) };
}

/// Run `f` with every [`map`] on this thread capped at the cores less [`KEPT_FOR_PLAY`]:
/// an import on a worker while a game ticks and draws, like Unity's background loads.
pub fn beside_play<R>(f: impl FnOnce() -> R) -> R {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CAP.set(self.0);
        }
    }
    let cap = cores().saturating_sub(KEPT_FOR_PLAY).max(1);
    let _restore = Restore(CAP.replace(Some(cap)));
    f()
}

/// Run `f` with every [`map`] on this thread using exactly `threads` threads: a
/// determinism probe.
pub fn with_threads<R>(threads: usize, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            PINNED.with(|p| p.set(self.0));
        }
    }
    let _restore = Restore(PINNED.with(|p| p.replace(Some(threads.max(1)))));
    f()
}

fn cores() -> usize {
    static CORES: OnceLock<usize> = OnceLock::new();
    *CORES.get_or_init(|| thread::available_parallelism().map_or(1, |n| n.get()))
}

fn threads_for(items: usize, work: usize) -> usize {
    let n = match PINNED.with(Cell::get) {
        Some(pinned) => pinned,
        None if work < SMALL => 1,
        None => CAP.get().map_or(cores(), |cap| cores().min(cap)),
    };
    n.clamp(1, items.max(1))
}

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
    fn beside_play_leaves_cores_free_and_is_undone_after() {
        let free = cores().saturating_sub(KEPT_FOR_PLAY).max(1);
        assert_eq!(beside_play(|| threads_for(1000, SMALL * 10)), free);
        assert_eq!(threads_for(1000, SMALL * 10), cores(), "undone");
        assert_eq!(
            beside_play(|| threads_for(8, SMALL - 1)),
            1,
            "small stays put"
        );
        let pinned = beside_play(|| with_threads(5, || threads_for(10, 0)));
        assert_eq!(pinned, 5, "a determinism probe's pin wins");
    }

    #[test]
    fn a_pin_is_undone_after_its_call() {
        with_threads(3, || assert_eq!(threads_for(10, 0), 3));
        assert_eq!(PINNED.with(Cell::get), None);
        assert_eq!(with_threads(0, || threads_for(10, 0)), 1);
        assert_eq!(with_threads(9, || threads_for(2, 0)), 2);
    }
}
