//! One channel's log. Bounded: an id no cursor has read past is not appended again; past [`cap`]
//! the older half goes, and a cursor that had not read it answers "rebuild", always correct.
//! `last` holds 8 bytes per id up to the highest stamped, until a `reset`.

#[derive(Debug, Default)]
pub(super) struct Log {
    pub revision: u64,
    pub recording: bool,
    entries: Vec<(u64, u32)>,
    /// The revision before the oldest entry kept: a cursor that read less is behind.
    pub floor: u64,
    /// The newest revision any cursor has read up to.
    high_water: u64,
    last: Vec<u64>,
    pub logged: u64,
    pub carried: Option<Carried>,
}

pub(super) type Carried = fn(&hecs::EntityRef<'_>) -> bool;

/// Entries kept: four stamps of every live entity, so a once-a-tick reader never falls behind a
/// tick that wrote everything, plus 4096 for churn in a small world.
pub(super) fn cap(live: usize) -> usize {
    4 * live + 4096
}

impl Log {
    pub fn new(carried: Option<Carried>) -> Self {
        Self {
            carried,
            ..Self::default()
        }
    }

    pub fn stamp(&mut self, id: u32, live: usize) {
        self.revision += 1;
        if !self.recording {
            return;
        }
        let at = id as usize;
        if at >= self.last.len() {
            self.last.resize(at + 1, 0);
        }
        if self.pending(id) {
            return;
        }
        self.last[at] = self.revision;
        self.entries.push((self.revision, id));
        self.logged += 1;
        if self.entries.len() > cap(live) {
            let half = self.entries.len() / 2;
            self.floor = self.entries[half - 1].0;
            self.entries.drain(..half);
        }
    }

    pub fn pending(&self, id: u32) -> bool {
        let last = self.last.get(id as usize).copied().unwrap_or(0);
        self.recording && last > self.high_water && last > self.floor
    }

    pub fn open(&mut self) -> u64 {
        if !self.recording {
            self.recording = true;
            self.floor = self.revision;
        }
        self.high_water = self.high_water.max(self.revision);
        self.revision
    }

    pub fn since(&mut self, seen: u64) -> Option<Vec<u32>> {
        if !self.recording || seen < self.floor {
            return None;
        }
        let start = self.entries.partition_point(|&(rev, _)| rev <= seen);
        let mut ids: Vec<u32> = self.entries[start..].iter().map(|&(_, id)| id).collect();
        ids.sort_unstable();
        ids.dedup();
        crate::walks::collected(ids.len());
        self.high_water = self.high_water.max(self.revision);
        Some(ids)
    }

    pub fn skip(&mut self, seen: u64) -> bool {
        let held = self.recording && seen >= self.floor;
        self.high_water = self.high_water.max(self.revision);
        held
    }

    pub fn reset(&mut self) {
        self.revision += 1;
        self.entries.clear();
        self.last.clear();
        self.floor = self.revision;
        self.high_water = self.revision;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
