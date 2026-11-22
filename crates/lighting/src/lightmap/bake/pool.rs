use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, Once, PoisonError};
use std::thread;

use super::super::progress::BakeProgress;

/// Items per batch: small, so a cancel lands at once; large, so the shared counter stays cold.
pub(in super::super) const BATCH: usize = 64;

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

// Sized once, never grown: each doubling frees a large block that macOS's allocator keeps in
// the footprint. `Slot::finish` sizes its own the same way.
fn flat<P>(batches: Vec<Vec<P>>) -> Vec<P> {
    let mut all = Vec::with_capacity(batches.iter().map(Vec::len).sum());
    batches.into_iter().for_each(|b| all.extend(b));
    all
}

struct Slot<P, L, T> {
    batches: Vec<Mutex<Vec<P>>>,
    ready: Once,
    left: AtomicUsize,
    parts: Mutex<Vec<(usize, Vec<L>)>>,
    done: Mutex<Option<T>>,
}

impl<P, L, T> Slot<P, L, T> {
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

    fn finish(&self, finish: impl FnOnce(Vec<Vec<P>>, Vec<L>) -> T) {
        let mut parts = std::mem::take(&mut *lock(&self.parts));
        parts.sort_unstable_by_key(|(batch, _)| *batch);
        let mut all = Vec::with_capacity(parts.iter().map(|(_, out)| out.len()).sum());
        parts.into_iter().for_each(|(_, out)| all.extend(out));
        let batches = self.batches.iter().map(|b| std::mem::take(&mut *lock(b)));
        *lock(&self.done) = Some(finish(batches.collect(), all));
    }
}

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
    run_slots(
        (slots, &|_| Vec::new()),
        threads,
        progress,
        first,
        trace,
        finish,
    )
}

type Make<'a, P> = dyn Fn(usize) -> Vec<P> + Sync + 'a;

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

fn lock<V>(m: &Mutex<V>) -> MutexGuard<'_, V> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}
