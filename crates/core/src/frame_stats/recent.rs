//! The recent ring behind [`FrameStats::window`](super::FrameStats::window): each
//! series' newest samples, stamped with the tick and drawn frame they were recorded
//! at, so a window holds only what was recorded inside it.

use std::collections::{BTreeMap, VecDeque};

use super::{Series, RECENT};

/// When a sample was recorded: the ring's tick and frame counts at that moment
/// (the tick or frame it belongs to, counted from 0), and whether a tick recorded it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Stamp {
    tick: u64,
    frame: u64,
    on_tick: bool,
}

/// The newest samples of every series in [`FrameStats`]' three maps, and the
/// clocks they are stamped on. Unlike the session's counts, both clocks move
/// outside Play too: the frame clock on every frame [`FrameStats::record_frame`] records.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Recent {
    /// Ticks and frames recorded since the last reset.
    pub(super) ticks: u64,
    pub(super) frames: u64,
    pub(super) metrics: BTreeMap<String, VecDeque<(Stamp, f64)>>,
    pub(super) systems: BTreeMap<String, VecDeque<(Stamp, f64)>>,
    pub(super) gpu_passes: BTreeMap<String, VecDeque<(Stamp, f64)>>,
    /// One stamp per tick, and one per frame that rendered: a window's counts.
    pub(super) tick_marks: VecDeque<Stamp>,
    pub(super) drawn_marks: VecDeque<Stamp>,
}

impl Recent {
    pub(super) fn stamp(&self, on_tick: bool) -> Stamp {
        let (tick, frame) = (self.ticks, self.frames);
        Stamp {
            tick,
            frame,
            on_tick,
        }
    }

    /// Whether `at` lies in the newest `n` frames and, for a tick's sample, in the
    /// newest `n` ticks too. So a sim metric leaves a window once `n` frames were
    /// drawn without a tick (edit mode, after Stop). A frame's sample is held to
    /// the frame clock alone: a headless run ticks many times per frame it renders.
    pub(super) fn within(&self, at: Stamp, n: u64) -> bool {
        n > 0 && at.frame + n >= self.frames && (!at.on_tick || at.tick + n >= self.ticks)
    }
}

/// Fold one sample into its session series (when it has one) and its ring.
pub(super) fn push(
    session: Option<&mut BTreeMap<String, Series>>,
    recent: &mut BTreeMap<String, VecDeque<(Stamp, f64)>>,
    at: Stamp,
    key: &str,
    value: f64,
) {
    if let Some(map) = session {
        match map.get_mut(key) {
            Some(series) => series.record(value),
            None => map.entry(key.to_string()).or_default().record(value),
        }
    }
    let samples = match recent.get_mut(key) {
        Some(samples) => samples,
        None => recent.entry(key.to_string()).or_default(),
    };
    bounded(samples, (at, value));
}

/// Push `item`, dropping the oldest past [`RECENT`].
pub(super) fn bounded<T>(ring: &mut VecDeque<T>, item: T) {
    if ring.len() == RECENT {
        ring.pop_front();
    }
    ring.push_back(item);
}

/// Each key's samples within the newest `n` frames, summarised as a fresh
/// [`Series`]; a key with none is left out.
pub(super) fn fold_recent(
    clocks: &Recent,
    recent: &BTreeMap<String, VecDeque<(Stamp, f64)>>,
    n: u64,
) -> BTreeMap<String, Series> {
    let mut out = BTreeMap::new();
    for (key, samples) in recent {
        let mut series = Series::default();
        for (at, value) in samples {
            if clocks.within(*at, n) {
                series.record(*value);
            }
        }
        if series.samples > 0 {
            out.insert(key.clone(), series);
        }
    }
    out
}
