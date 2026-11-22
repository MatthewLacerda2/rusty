use std::collections::{BTreeMap, VecDeque};

use super::{Series, RECENT};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Stamp {
    tick: u64,
    frame: u64,
    on_tick: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Recent {
    pub(super) ticks: u64,
    pub(super) frames: u64,
    pub(super) metrics: BTreeMap<String, VecDeque<(Stamp, f64)>>,
    pub(super) systems: BTreeMap<String, VecDeque<(Stamp, f64)>>,
    pub(super) gpu_passes: BTreeMap<String, VecDeque<(Stamp, f64)>>,
    pub(super) tick_marks: VecDeque<Stamp>,
    pub(super) drawn_marks: VecDeque<Stamp>,
    pub(super) open: Option<(Stamp, bool)>,
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
        n > 0 && !self.past_edge(at, n) && (!at.on_tick || at.tick + n >= self.ticks)
    }

    /// Whether `at` is older than the newest `n` frames. Stamps in a ring only
    /// grow, so this is false then true walking back: a ring's window starts at the
    /// first stamp it is false for, found by a binary search.
    pub(super) fn past_edge(&self, at: Stamp, n: u64) -> bool {
        at.frame + n < self.frames
    }
}

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

pub(super) fn bounded<T>(ring: &mut VecDeque<T>, item: T) {
    if ring.len() == RECENT {
        ring.pop_front();
    }
    ring.push_back(item);
}

pub(super) fn fold_recent(
    clocks: &Recent,
    recent: &BTreeMap<String, VecDeque<(Stamp, f64)>>,
    n: u64,
    keep: &impl Fn(Stamp) -> bool,
) -> BTreeMap<String, Series> {
    let mut out = BTreeMap::new();
    for (key, samples) in recent {
        let edge = samples.partition_point(|(at, _)| clocks.past_edge(*at, n));
        let mut series = Series::default();
        for (at, value) in samples.range(edge..) {
            if keep(*at) {
                series.record(*value);
            }
        }
        if series.samples > 0 {
            out.insert(key.clone(), series);
        }
    }
    out
}
