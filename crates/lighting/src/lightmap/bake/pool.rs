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
//! [`run`] cuts plain charts into batches and joins them back for its `finish`,
//! and [`run_made`] does the same for charts it makes only when the first worker
//! reaches them (#2312), so a chart's items live from its first batch to its
//! `finish`, never from the bake's start.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, Once, PoisonError};
use std::thread;

use super::super::progress::BakeProgress;

/// Items in one batch: small enough that a cancel lands at once and the queue's
/// tail stays short, large enough that the shared counter stays cold.
pub(in super::super) const BATCH: usize = 64;

/// `items` cut into batches of [`BATCH`], in order; none for no item.
pub(crate) fn batches<P>(items: impl IntoIterator<Item = P>) -> Vec<Vec<P>> {
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

/// `batches` as one list, sized once (`Slot::finish`'s reason).
fn flat<P>(batches: Vec<Vec<P>>) -> Vec<P> {
    let mut all = Vec::with_capacity(batches.iter().map(Vec::len).sum());
    batches.into_iter().for_each(|b| all.extend(b));
    all
}

/// One chart's batches (taken by its `finish`), their results as they come back,
/// and its result once the last one has.
struct Slot<P, L, T> {
    batches: Vec<Mutex<Vec<P>>>,
    /// Done once the batches hold the chart's items: at once for a chart cut
    /// before the run, at its first batch for one [`run_made`] makes.
    ready: Once,
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
        let ready = Once::new();
        ready.call_once(|| {});
        Self {
            left: AtomicUsize::new(batches.len()),
            batches: batches.into_iter().map(Mutex::new).collect(),
            ready,
            parts: Mutex::default(),
            done: Mutex::new(None),
        }
    }

    /// A chart of `count` items not made yet: its batches stand empty until
    /// [`Slot::fill`].
    fn later(count: usize) -> Self {
        let batches = count.div_ceil(BATCH).max(1);
        Self {
            left: AtomicUsize::new(batches),
            batches: (0..batches).map(|_| Mutex::new(Vec::new())).collect(),
            ready: Once::new(),
            parts: Mutex::default(),
            done: Mutex::new(None),
        }
    }

    /// The chart's batches made by `make`, once, by the first worker to reach the
    /// chart; the others wait for it. `make` gives the count [`Slot::later`] was
    /// told, so the batches line up.
    fn fill(&self, make: impl FnOnce() -> Vec<P>) {
        self.ready.call_once(|| {
            let made = batches(make());
            debug_assert!(
                made.len() <= self.batches.len(),
                "a chart made larger than counted"
            );
            for (slot, batch) in self.batches.iter().zip(made) {
                *lock(slot) = batch;
            }
        });
    }

    /// Once every batch is in: the chart's batches move out (no worker holds one)
    /// with its results in item order, so both are freed when `finish` is done.
    fn finish(&self, finish: impl FnOnce(Vec<Vec<P>>, Vec<L>) -> T) {
        let mut parts = std::mem::take(&mut *lock(&self.parts));
        parts.sort_unstable_by_key(|(batch, _)| *batch);
        // Sized once, never grown by doubling (#2312): each growth frees a large
        // block, which macOS's allocator keeps in the footprint.
        let mut all = Vec::with_capacity(parts.iter().map(|(_, out)| out.len()).sum());
        parts.into_iter().for_each(|(_, out)| all.extend(out));
        let batches = self.batches.iter().map(|b| std::mem::take(&mut *lock(b)));
        *lock(&self.done) = Some(finish(batches.collect(), all));
    }
}

/// Run `trace(chart, first, items)` over every batch of every chart on `threads`
/// workers (`first` the batch's first item's index in its chart), then
/// `finish(chart, items, results)` once per chart with its own items back and its
/// results in item order. `None` when `progress` was cancelled before every chart
/// finished.
pub(crate) fn run<P, L, T>(
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
    let trace =
        |c: usize, b: usize, items: &mut [P]| (trace(c, b * BATCH, items), items.len() as u64);
    let finish = |c: usize, items: Vec<Vec<P>>, out: Vec<L>| finish(c, flat(items), out);
    run_batched(charts, threads, progress, &|_, _| false, trace, finish)
}

/// [`run`] over charts made only when the first worker reaches them (#2312):
/// `counts` says how many items each chart has, and `make(chart)` makes them
/// (the same count, every time). A chart's items so live from its first batch to
/// its `finish`, so the run holds the charts in flight, never every chart at once;
/// results are [`run`]'s.
pub(crate) fn run_made<P, L, T>(
    counts: &[usize],
    make: impl Fn(usize) -> Vec<P> + Sync,
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
    let slots = counts.iter().map(|&n| Slot::later(n)).collect();
    let trace =
        |c: usize, b: usize, items: &mut [P]| (trace(c, b * BATCH, items), items.len() as u64);
    let finish = |c: usize, items: Vec<Vec<P>>, out: Vec<L>| finish(c, flat(items), out);
    run_slots(
        (slots, &make),
        threads,
        progress,
        &|_, _| false,
        trace,
        finish,
    )
}

/// [`run`] over charts already cut into batches ([`batches`]), each traced as
/// `trace(chart, batch, &mut items)` so it may update its items in place, which
/// gives its results and the items it counts as traced (a progressive step passes
/// over some batches without tracing them), and
/// the batches `first(chart, batch)` picks queued ahead of the rest, each group
/// in chart order (#1970's Prioritize View). Only the order the batches are
/// taken in moves: results still come back in item order.
pub(crate) fn run_batched<P, L, T>(
    charts: Vec<Vec<Vec<P>>>,
    threads: usize,
    progress: &BakeProgress,
    first: &(dyn Fn(usize, usize) -> bool + Sync),
    trace: impl Fn(usize, usize, &mut [P]) -> (Vec<L>, u64) + Sync,
    finish: impl Fn(usize, Vec<Vec<P>>, Vec<L>) -> T + Sync,
) -> Option<Vec<T>>
where
    P: Send,
    L: Send,
    T: Send,
{
    let slots = charts.into_iter().map(Slot::new).collect();
    // Every chart is made already, so nothing is ever made.
    run_slots(
        (slots, &|_| Vec::new()),
        threads,
        progress,
        first,
        trace,
        finish,
    )
}

/// How a chart's items are made from its index ([`run_made`]).
type Make<'a, P> = dyn Fn(usize) -> Vec<P> + Sync + 'a;

/// The pool over `slots`, each chart made by `make` when its first batch is taken
/// (a no-op for one cut before the run): [`run_batched`]'s contract.
fn run_slots<P, L, T>(
    (slots, make): (Vec<Slot<P, L, T>>, &Make<P>),
    threads: usize,
    progress: &BakeProgress,
    first: &(dyn Fn(usize, usize) -> bool + Sync),
    trace: impl Fn(usize, usize, &mut [P]) -> (Vec<L>, u64) + Sync,
    finish: impl Fn(usize, Vec<Vec<P>>, Vec<L>) -> T + Sync,
) -> Option<Vec<T>>
where
    P: Send,
    L: Send,
    T: Send,
{
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
        slot.fill(|| make(c));
        let (out, traced) = trace(c, b, &mut lock(&slot.batches[b]));
        progress.advance(traced);
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
