//! The bake's one work pool (#1656): every chart's texels cut into fixed-size
//! batches in one queue, and every worker pulling the next batch off a shared
//! counter until the queue is empty. Small meshes no longer leave cores idle, a
//! costly corner no longer holds up a whole mesh, and threads are spawned once per
//! bake instead of once per mesh.
//!
//! A chart is finished (its per-chart post-pass, `finish`) by whichever worker
//! traces its last batch, so the post-passes run in parallel too and a chart's
//! traced texels are freed as soon as it is done. Results come back in chart order,
//! each chart's items in their own order: which worker ran a batch never shows.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread;

use super::progress::BakeProgress;

/// Items in one batch: small enough that a cancel lands at once and the queue's
/// tail stays short, large enough that the shared counter stays cold.
pub(super) const BATCH: usize = 64;

/// One chart's batches as they come back, and its result once the last one has.
struct Slot<L, T> {
    left: AtomicUsize,
    parts: Mutex<Vec<(usize, Vec<L>)>>,
    done: Mutex<Option<T>>,
}

/// Run `trace(chart, first, items)` over every batch of every chart on `threads`
/// workers, then `finish(chart, results)` once per chart with its results in item
/// order. `None` when `progress` was cancelled before every chart finished.
pub(super) fn run<P, L, T>(
    charts: &[&[P]],
    threads: usize,
    progress: &BakeProgress,
    trace: impl Fn(usize, usize, &[P]) -> Vec<L> + Sync,
    finish: impl Fn(usize, Vec<L>) -> T + Sync,
) -> Option<Vec<T>>
where
    P: Sync,
    L: Send,
    T: Send,
{
    // An empty chart still gets one (empty) batch, so it is finished like the rest.
    let batches: Vec<(usize, usize)> = charts
        .iter()
        .enumerate()
        .flat_map(|(c, items)| (0..items.len().max(1)).step_by(BATCH).map(move |s| (c, s)))
        .collect();
    let slots: Vec<Slot<L, T>> = charts
        .iter()
        .map(|items| Slot {
            left: AtomicUsize::new(items.len().div_ceil(BATCH).max(1)),
            parts: Mutex::default(),
            done: Mutex::new(None),
        })
        .collect();
    let next = AtomicUsize::new(0);
    let work = || loop {
        if progress.is_cancelled() {
            return;
        }
        let Some(&(c, first)) = batches.get(next.fetch_add(1, Ordering::Relaxed)) else {
            return;
        };
        let items = &charts[c][first..(first + BATCH).min(charts[c].len())];
        let out = trace(c, first, items);
        progress.advance(items.len() as u64);
        let slot = &slots[c];
        lock(&slot.parts).push((first, out));
        // The worker that hands in a chart's last batch finishes it.
        if slot.left.fetch_sub(1, Ordering::AcqRel) == 1 {
            let mut parts = std::mem::take(&mut *lock(&slot.parts));
            parts.sort_unstable_by_key(|(first, _)| *first);
            let all = parts.into_iter().flat_map(|(_, out)| out).collect();
            *lock(&slot.done) = Some(finish(c, all));
        }
    };
    let threads = threads.clamp(1, batches.len().max(1));
    thread::scope(|scope| {
        for _ in 1..threads {
            scope.spawn(&work);
        }
        work();
    });
    slots
        .into_iter()
        .map(|slot| {
            slot.done
                .into_inner()
                .unwrap_or_else(PoisonError::into_inner)
        })
        .collect()
}

/// A worker that panicked takes the whole bake down with it (the scope re-raises
/// it), so a poisoned lock is never read past; this only keeps the type simple.
fn lock<V>(m: &Mutex<V>) -> MutexGuard<'_, V> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}
