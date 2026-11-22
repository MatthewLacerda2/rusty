use rusty_components::{Bounds, MontageEvent, Motion, Playback, Playhead, Step};

use super::blend_tree::dominant_leaf;
use super::motion::MotionContext;

pub const END_EVENT: &str = "End";

/// Loops of one marker fired in a single step, at most, so an absurd speed on a short clip can't
/// flood the scripts.
const MAX_CROSSINGS: i64 = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct Crossed {
    pub name: String,
    /// How far into the step it was crossed, in `[0, 1]`.
    pub at: f32,
    pub layer: usize,
    pub montage: Option<MontageEvent>,
}

/// A stable sort: events tied on time and layer keep the authored order they were found in.
pub fn sort(events: &mut [Crossed]) {
    events.sort_by(|a, b| a.at.total_cmp(&b.at).then(a.layer.cmp(&b.layer)));
}

impl MotionContext<'_> {
    pub fn crossed(
        &self,
        playback: &Playback,
        playhead: Playhead,
        step: Step,
        layer: usize,
    ) -> Vec<Crossed> {
        let span = step.span;
        let bounds = playhead.bounds(playback.loop_clip);
        let mut marks: Vec<(f32, &str)> = self.marks(playback);
        if let Bounds::Clamp(end) = bounds {
            // A clamped playhead never passes the clip's ends, so a marker beyond
            // them sits on the end (Unity keeps events within the clip).
            for (mark, _) in &mut marks {
                *mark = mark.clamp(0.0, end);
            }
        }
        let mut out = Vec::new();
        let (fired, travel) = (step.events, span.1 - span.0);
        let into = |at: f32| match travel != 0.0 && fired != span {
            true => ((fired.0 + at * (fired.1 - fired.0) - span.0) / travel).clamp(0.0, 1.0),
            false => at,
        };
        for (mark, name) in marks {
            for at in crossings(fired, mark, bounds) {
                out.push(Crossed {
                    name: name.to_string(),
                    at: into(at),
                    layer,
                    montage: None,
                });
            }
        }
        if let (Bounds::Clamp(end), true) = (bounds, step.end) {
            let at = if span.1 > span.0 {
                ((end - span.0) / (span.1 - span.0)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            out.push(Crossed {
                name: END_EVENT.to_string(),
                at,
                layer,
                montage: None,
            });
        }
        out
    }

    fn marks(&self, playback: &Playback) -> Vec<(f32, &str)> {
        let Some(machine) = self.machine else {
            return Vec::new();
        };
        match playback.motion() {
            Motion::Clip(clip) => playback
                .current_node
                .as_deref()
                .and_then(|n| machine.node(n))
                .filter(|n| n.blend_tree.is_none() && n.clip == clip)
                .map(|n| n.events.iter().map(|e| (e.time, e.name.as_str())).collect())
                .unwrap_or_default(),
            Motion::Tree(node) => {
                let Some(tree) = machine.node(node).and_then(|n| n.blend_tree.as_ref()) else {
                    return Vec::new();
                };
                let resolves = |clip: &str| self.clip(clip).is_some();
                let Some(leaf) = dominant_leaf(tree, self.parameters, &resolves) else {
                    return Vec::new();
                };
                let length = self.clip(leaf.clip).map_or(0.0, |c| c.duration);
                if length <= 0.0 {
                    return Vec::new();
                }
                leaf.events
                    .iter()
                    .map(|e| (e.time / length, e.name.as_str()))
                    .collect()
            }
        }
    }
}

fn crossings((from, to): (f32, f32), mark: f32, bounds: Bounds) -> Vec<f32> {
    let span = to - from;
    if span == 0.0 || !span.is_finite() {
        return Vec::new();
    }
    let (lo, hi) = match bounds {
        Bounds::Clamp(end) => (to <= 0.0 && mark == 0.0, to >= end && mark == end),
        _ => (false, false),
    };
    let (first, last) = match (bounds, span > 0.0) {
        (Bounds::Wrap(w), true) => (
            ((from - mark) / w).ceil() as i64,
            ((to - mark) / w).ceil() as i64 - 1,
        ),
        (Bounds::Wrap(w), false) => (
            ((to - mark) / w).floor() as i64 + 1,
            ((from - mark) / w).floor() as i64,
        ),
        (_, true) => (0, i64::from(from <= mark && (mark < to || hi)) - 1),
        (_, false) => (0, i64::from((to < mark || lo) && mark <= from) - 1),
    };
    let wrap = match bounds {
        Bounds::Wrap(w) => w,
        _ => 0.0,
    };
    let last = last.min(first + MAX_CROSSINGS - 1);
    (first..=last)
        .map(|k| {
            let at = mark + k as f32 * wrap;
            ((at - from) / span).clamp(0.0, 1.0)
        })
        .collect()
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod events_tests;
