use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use rusty_render::GpuTimes;

use super::record::FrameRecord;

pub(super) const HOLD: u32 = 4;

pub(super) struct Entry {
    pub(super) record: Option<FrameRecord>,
    pub(super) wants_gpu: bool,
    pub(super) rest: Vec<String>,
    pub(super) tick: u64,
    ms: Option<f64>,
    gpu: Option<GpuTimes>,
    age: u32,
}

impl Entry {
    pub(super) fn new(
        record: Option<FrameRecord>,
        wants_gpu: bool,
        rest: Vec<String>,
        tick: u64,
    ) -> Self {
        let wants_gpu = wants_gpu && record.as_ref().is_some_and(|r| r.cpu_ms.is_some());
        let (ms, gpu, age) = (None, None, 0);
        Self {
            record,
            wants_gpu,
            rest,
            tick,
            ms,
            gpu,
            age,
        }
    }

    fn whole(&self) -> bool {
        self.record.is_none() || (self.ms.is_some() && (self.gpu.is_some() || !self.wants_gpu))
    }

    fn lines(&self) -> Vec<String> {
        let record = self.record.as_ref();
        let line = record.map(|r| r.line(self.ms, self.gpu.as_ref(), self.wants_gpu));
        line.into_iter().chain(self.rest.iter().cloned()).collect()
    }
}

#[derive(Default)]
pub(super) struct Held {
    entries: VecDeque<Entry>,
}

impl Held {
    pub(super) fn ended(&mut self, wall_ms: f64) {
        if let Some(newest) = self.entries.back_mut() {
            newest.ms.get_or_insert(wall_ms);
        }
    }

    pub(super) fn gpu(&mut self, times: GpuTimes) {
        let measured = self.entries.iter_mut().find(|e| {
            e.wants_gpu
                && e.gpu.is_none()
                && e.record.as_ref().is_some_and(|r| r.frame == times.frame)
        });
        if let Some(entry) = measured {
            entry.gpu = Some(times);
        }
    }

    pub(super) fn push(&mut self, entry: Entry) -> Vec<(Vec<String>, u64)> {
        for held in &mut self.entries {
            held.age += 1;
        }
        self.entries.push_back(entry);
        let mut out = Vec::new();
        while let Some(front) = self.entries.front() {
            if !front.whole() && front.age < HOLD {
                break;
            }
            out.extend(self.entries.pop_front().map(|e| (e.lines(), e.tick)));
        }
        out
    }

    pub(super) fn drain(&mut self) -> Vec<(Vec<String>, u64)> {
        self.entries
            .drain(..)
            .map(|e| (e.lines(), e.tick))
            .collect()
    }
}

pub(super) type Shared = Arc<Mutex<Held>>;

static RUNNING: Mutex<Vec<(PathBuf, Weak<Mutex<Held>>)>> = Mutex::new(Vec::new());

pub(super) fn register(dir: &Path) -> Shared {
    let held = Shared::default();
    let mut running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
    running.retain(|(_, weak)| weak.strong_count() > 0);
    running.push((dir.to_path_buf(), Arc::downgrade(&held)));
    held
}

pub(super) fn lines(dir: &Path) -> Vec<String> {
    let running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
    let logs = running
        .iter()
        .filter(|(d, _)| d == dir)
        .filter_map(|(_, w)| w.upgrade());
    let mut out = Vec::new();
    for held in logs {
        let held = held.lock().unwrap_or_else(PoisonError::into_inner);
        out.extend(held.entries.iter().flat_map(Entry::lines));
    }
    out
}

#[cfg(test)]
#[path = "held_tests.rs"]
mod tests;
