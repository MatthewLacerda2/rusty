//! src/ecs/changes/log.rs — one channel's change log: a revision, and the ids
//! stamped since the oldest point a cursor can still read from.
//!
//! **Bounded.** An id already in the log that no cursor has read past is not
//! appended again, so between two reads the log grows by at most one entry per
//! distinct id stamped, and a channel nobody reads holds at most one entry per id
//! ever stamped. Past [`cap`] entries (a consumer that reads, then stops, while
//! another keeps reading) the older half is dropped; a cursor that had not read it
//! answers "rebuild everything" next time, which is always correct.
//!
//! That bound is on `entries`. Beside them a recording log keeps `last`, 8 bytes per
//! id up to the highest id it ever stamped (24 KiB for a 3000-entity level), until
//! the World is cleared or rewound.

/// One channel's log.
#[derive(Debug, Default)]
pub(super) struct Log {
    /// Bumped by every stamp, recorded or not: what [`super::Cursor::seen`] reads up to.
    pub revision: u64,
    /// Whether a cursor was ever opened: until then a stamp only bumps `revision`.
    pub recording: bool,
    /// `(revision, id)` per recorded stamp, oldest first.
    entries: Vec<(u64, u32)>,
    /// The revision before the oldest entry kept: a cursor that read less is behind.
    pub floor: u64,
    /// The newest revision any cursor has read up to.
    high_water: u64,
    /// Per id, the revision of its newest entry (`0`: none), so a repeat is skipped.
    last: Vec<u64>,
    /// Entries ever appended: a repeat no cursor has read past is not one, so with a
    /// reader every tick its growth per tick is the distinct ids dirtied in it.
    pub logged: u64,
    /// Whether whole-entity writes (place, overwrite, remove) stamp this channel:
    /// true for a component column, false for a derived channel stamped by hand.
    pub column: bool,
}

/// The most entries a log keeps when `live` entities exist: four stamps of every
/// live entity, so a consumer reading once a tick never falls behind a tick that
/// wrote everything, plus room for churn in a small world.
pub(super) fn cap(live: usize) -> usize {
    4 * live + 4096
}

impl Log {
    pub fn new(column: bool) -> Self {
        Self {
            column,
            ..Self::default()
        }
    }

    /// `id` changed on this channel.
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

    /// Whether `id` has an entry still in the log that no cursor has read past.
    pub fn pending(&self, id: u32) -> bool {
        let last = self.last.get(id as usize).copied().unwrap_or(0);
        self.recording && last > self.high_water && last > self.floor
    }

    /// A cursor opens here: recording starts, and it has read everything so far.
    pub fn open(&mut self) -> u64 {
        if !self.recording {
            self.recording = true;
            self.floor = self.revision;
        }
        self.high_water = self.high_water.max(self.revision);
        self.revision
    }

    /// The ids stamped after revision `seen`, ascending and each once; `None` when
    /// the log no longer holds all of them.
    pub fn since(&mut self, seen: u64) -> Option<Vec<u32>> {
        if !self.recording || seen < self.floor {
            return None;
        }
        let start = self.entries.partition_point(|&(rev, _)| rev <= seen);
        let mut ids: Vec<u32> = self.entries[start..].iter().map(|&(_, id)| id).collect();
        ids.sort_unstable();
        ids.dedup();
        self.high_water = self.high_water.max(self.revision);
        Some(ids)
    }

    /// The World was replaced: forget every entry, keep the revision moving.
    pub fn reset(&mut self) {
        self.revision += 1;
        self.entries.clear();
        self.last.clear();
        self.floor = self.revision;
        self.high_water = self.revision;
    }

    /// Entries held now: what [`cap`] bounds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
