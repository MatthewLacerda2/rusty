//! A frame's lines held until they are whole (#2206): its wall time is known when
//! the next frame starts, and its GPU times resolve a frame or two after it drew,
//! so the writer keeps each frame here and writes its line with its own `ms`,
//! `gpu_ms` and `gpu_passes`, never a neighbour's.
//!
//! * **Bounded.** A frame is written once what belongs to it is in, or after
//!   [`HOLD`] later frames, whatever is missing then named in its `unresolved`
//!   field (`ms`, `gpu`). An adapter without timestamps waits on no GPU times.
//! * **In order.** A frame's console, load and audio lines wait with it, and a
//!   frame is written only after every earlier one, so the files stay in time
//!   order (the read stops at the first line older than its look-back).
//! * **Read from memory.** What is held is still the newest of the log: the read
//!   takes it as it stands ([`lines`]), so `Debug.FrameLog{ last_seconds }` returns
//!   the newest frames, those not yet whole saying what they lack.
//! * **Flushed when the log stops** (`FrameLog`'s drop), as it stands.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use rusty_render::GpuTimes;

use super::record::FrameRecord;

/// Later frames a frame waits for its GPU times: past what the renderer's
/// readback ring takes to resolve one (a frame or two) on any adapter it has met.
pub(super) const HOLD: u32 = 4;

/// One frame's lines as they wait.
pub(super) struct Entry {
    /// Its `frames` record (`None` while that channel is off).
    pub(super) record: Option<FrameRecord>,
    /// Whether its record waits on GPU times: it drew, on an adapter that times.
    pub(super) wants_gpu: bool,
    /// Its console, load and audio lines, written after its record.
    pub(super) rest: Vec<String>,
    /// Its tick, for a drop note.
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

    /// Its lines as they stand.
    fn lines(&self) -> Vec<String> {
        let record = self.record.as_ref();
        let line = record.map(|r| r.line(self.ms, self.gpu.as_ref(), self.wants_gpu));
        line.into_iter().chain(self.rest.iter().cloned()).collect()
    }
}

/// The frames held, oldest first.
#[derive(Default)]
pub(super) struct Held {
    entries: VecDeque<Entry>,
}

impl Held {
    /// A frame starts after `wall_ms`: the newest held frame's own wall time.
    pub(super) fn ended(&mut self, wall_ms: f64) {
        if let Some(newest) = self.entries.back_mut() {
            newest.ms.get_or_insert(wall_ms);
        }
    }

    /// GPU times resolved: onto the drawn frame they measure, if still held.
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

    /// Hold `entry`, then return, oldest first, the frames now to write (lines and
    /// tick): every whole one at the front, and any held [`HOLD`] frames.
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

    /// Every frame held, as it stands, to write when the log stops.
    pub(super) fn drain(&mut self) -> Vec<(Vec<String>, u64)> {
        self.entries
            .drain(..)
            .map(|e| (e.lines(), e.tick))
            .collect()
    }
}

/// A log's held frames, shared with reads of its folder.
pub(super) type Shared = Arc<Mutex<Held>>;

/// The running logs' held frames by folder, so a read of a folder finds them.
static RUNNING: Mutex<Vec<(PathBuf, Weak<Mutex<Held>>)>> = Mutex::new(Vec::new());

/// A new, empty hold for a log in `dir`, found by [`lines`] while it lives.
pub(super) fn register(dir: &Path) -> Shared {
    let held = Shared::default();
    let mut running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
    running.retain(|(_, weak)| weak.strong_count() > 0);
    running.push((dir.to_path_buf(), Arc::downgrade(&held)));
    held
}

/// The lines a running log in `dir` still holds, oldest first, as they stand.
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
