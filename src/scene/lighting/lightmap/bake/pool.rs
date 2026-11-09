//! The bake's one work pool (#1656): every chart's texels cut into fixed-size
//! batches in one queue, and every worker pulling the next batch off a shared
//! counter until the queue is empty. Small meshes no longer leave cores idle, a
//! costly corner no longer holds up a whole mesh, and threads are spawned once per
//! bake instead of once per mesh.
//!
//! A chart is finished (its per-chart post-pass, `finish`) by whichever worker
//! traces its last batch, so the post-passes run in parallel too. The pool owns
//! the charts' items and hands each chart's to its `finish`, so a chart's items and
//! traced texels are freed as soon as it is done, not when the bake ends. Results come back in chart order,
//! each chart's items in their own order: which worker ran a batch never shows.
//!
//! The pool's own shape is [`run_batched`]: every chart already cut into its
//! batches, each traced through `&mut`, so a caller that keeps state per item
//! between runs (#1970's progressive bake) updates it in place and holds one copy.
//! [`run`] cuts plain charts into batches and joins them back for its `finish`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread;

use super::super::progress::BakeProgress;

/// Items in one batch: small enough that a cancel lands at once and the queue's
/// tail stays short, large enough that the shared counter stays cold.
pub(in super::super) const BATCH: usize = 64;

/// `items` cut into batches of [`BATCH`], in order; none for no item.
pub(in crate::scene::lighting) fn batches<P>(items: impl IntoIterator<Item = P>) -> Vec<Vec<P>> {
    let mut items = items.into_iter();
    let mut out = Vec::new();
    loop {
        let batch: Vec<P> = items.by_ref().take(BATCH).collect();
        if batch.is_empty() {
            return out;
        }
        out.push(batch);
    }
}

/// One chart's batches (taken by its `finish`), their results as they come back,
/// and its result once the last one has.
struct Slot<P, L, T> {
    batches: Vec<Mutex<Vec<P>>>,
    left: AtomicUsize,
    parts: Mutex<Vec<(usize, Vec<L>)>>,
    done: Mutex<Option<T>>,
}

impl<P, L, T> Slot<P, L, T> {
    /// A chart of `batches`; a chart with none gets one empty batch, so it is
    /// finished like the rest.
    fn new(mut batches: Vec<Vec<P>>) -> Self {
        if batches.is_empty() {
            batches.push(Vec::new());
        }
        Self {
            left: AtomicUsize::new(batches.len()),
            batches: batches.into_iter().map(Mutex::new).collect(),
            parts: Mutex::default(),
            done: Mutex::new(None),
        }
    }

    /// Once every batch is in: the chart's batches move out (no worker holds one)
    /// with its results in item order, so both are freed when `finish` is done.
    fn finish(&self, finish: impl FnOnce(Vec<Vec<P>>, Vec<L>) -> T) {
        let mut parts = std::mem::take(&mut *lock(&self.parts));
        parts.sort_unstable_by_key(|(batch, _)| *batch);
        let all = parts.into_iter().flat_map(|(_, out)| out).collect();
        let batches = self.batches.iter().map(|b| std::mem::take(&mut *lock(b)));
        *lock(&self.done) = Some(finish(batches.collect(), all));
    }
}

/// Run `trace(chart, first, items)` over every batch of every chart on `threads`
/// workers (`first` the batch's first item's index in its chart), then
/// `finish(chart, items, results)` once per chart with its own items back and its
/// results in item order. `None` when `progress` was cancelled before every chart
/// finished.
pub(in crate::scene::lighting) fn run<P, L, T>(
    charts: Vec<Vec<P>>,
    threads: usize,
    progress: &BakeProgress,
    trace: impl Fn(usize, usize, &[P]) -> Vec<L> + Sync,
    finish: impl Fn(usize, Vec<P>, Vec<L>) -> T + Sync,
) -> Option<Vec<T>>
where
    P: Send,
    L: Send,
    T: Send,
{
    let charts = charts.into_iter().map(batches).collect();
    let trace = |c: usize, b: usize, items: &mut [P]| trace(c, b * BATCH, items);
    let finish = |c: usize, items: Vec<Vec<P>>, out: Vec<L>| {
        finish(c, items.into_iter().flatten().collect(), out)
    };
    run_batched(charts, threads, progress, &|_, _| false, trace, finish)
}

/// [`run`] over charts already cut into batches ([`batches`]), each traced as
/// `trace(chart, batch, &mut items)` so it may update its items in place, and
/// the batches `first(chart, batch)` picks queued ahead of the rest, each group
/// in chart order (#1970's Prioritize View). Only the order the batches are
/// taken in moves: results still come back in item order.
pub(in crate::scene::lighting) fn run_batched<P, L, T>(
    charts: Vec<Vec<Vec<P>>>,
    threads: usize,
    progress: &BakeProgress,
    first: &(dyn Fn(usize, usize) -> bool + Sync),
    trace: impl Fn(usize, usize, &mut [P]) -> Vec<L> + Sync,
    finish: impl Fn(usize, Vec<Vec<P>>, Vec<L>) -> T + Sync,
) -> Option<Vec<T>>
where
    P: Send,
    L: Send,
    T: Send,
{
    let slots: Vec<Slot<P, L, T>> = charts.into_iter().map(Slot::new).collect();
    let mut queue: Vec<(usize, usize)> = (slots.iter().enumerate())
        .flat_map(|(c, slot)| (0..slot.batches.len()).map(move |b| (c, b)))
        .collect();
    queue.sort_by_key(|&(c, b)| !first(c, b));
    let next = AtomicUsize::new(0);
    let work = || loop {
        if progress.is_cancelled() {
            return;
        }
        let Some(&(c, b)) = queue.get(next.fetch_add(1, Ordering::Relaxed)) else {
            return;
        };
        let slot = &slots[c];
        let (out, traced) = {
            let mut items = lock(&slot.batches[b]);
            (trace(c, b, &mut items), items.len())
        };
        progress.advance(traced as u64);
        lock(&slot.parts).push((b, out));
        // The worker that hands in a chart's last batch finishes it.
        if slot.left.fetch_sub(1, Ordering::AcqRel) == 1 {
            slot.finish(|items, all| finish(c, items, all));
        }
    };
    let threads = threads.clamp(1, queue.len().max(1));
    thread::scope(|scope| {
        for _ in 1..threads {
            scope.spawn(work);
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
