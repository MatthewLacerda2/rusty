use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

pub trait OcclusionObserver: Send + Sync {
    fn cells(&self, total: usize);
    fn cell_started(&self, cell: usize);
    fn cell_finished(&self, cell: usize);
}

#[derive(Clone, Default)]
struct Observer(Option<Arc<dyn OcclusionObserver>>);

impl fmt::Debug for Observer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0.is_some() {
            "observed"
        } else {
            "unobserved"
        })
    }
}

#[derive(Debug, Default)]
pub struct OcclusionProgress {
    done: AtomicUsize,
    total: AtomicUsize,
    cancelled: AtomicBool,
    observer: Observer,
}

impl OcclusionProgress {
    pub fn observed(observer: Arc<dyn OcclusionObserver>) -> Self {
        Self {
            observer: Observer(Some(observer)),
            ..Self::default()
        }
    }

    pub fn cells(&self) -> (usize, usize) {
        let (done, total) = (&self.done, &self.total);
        (done.load(Ordering::Relaxed), total.load(Ordering::Relaxed))
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub(super) fn start(&self, total: usize, sampled: usize) {
        self.total.store(sampled, Ordering::Relaxed);
        if let Some(o) = &self.observer.0 {
            o.cells(total);
        }
    }

    pub(super) fn cell_started(&self, cell: usize) {
        if let Some(o) = &self.observer.0 {
            o.cell_started(cell);
        }
    }

    pub(super) fn cell_done(&self, cell: usize) {
        if let Some(o) = &self.observer.0 {
            o.cell_finished(cell);
        }
        self.done.fetch_add(1, Ordering::Relaxed);
    }
}
