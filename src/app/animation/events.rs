//! src/app/animation/events.rs — finding the animation events a step crosses (#459).
//!
//! Each fixed step a layer's playhead travels from `from` to `to` (before any
//! loop wrap). A marker at `mark` fires once for every position `mark + k × wrap`
//! that travel passes, over the half-open span `[from, to)` (mirrored when
//! playing backwards): so a loop wrap, a step large enough to skip a marker, and
//! a marker right at the start each fire exactly once, and a playhead standing
//! still (speed 0) fires nothing. Which markers count:
//! - only the **current** motion's: a motion being faded out by a crossfade is
//!   silent (the one coming in fires from its first frame);
//! - in a blend tree, only the **dominant** child's — the highest weight, the
//!   first authored on a tie — so a walk/run blend plays one footstep, not two;
//! - on an extra layer, only while its weight is above 0;
//! - plus the automatic [`END_EVENT`] when a non-looping motion reaches its end
//!   playing forwards: once per pass, since its playhead clamps there (#1362),
//!   and also on the step a clip's length becomes known with the playhead
//!   already past it, and on a zero-length clip's first step ([`Step::end`],
//!   #1386).
//!
//! Every step's events are ordered by when in the step they were crossed, then
//! by layer, then authored order — deterministic, and true time order.

use crate::components::{Bounds, Motion, Playback, Playhead, Step};

use super::blend_tree::tree_weights;
use super::motion::MotionContext;

/// The automatic event a non-looping clip (or blend tree) fires when its playhead
/// reaches the end.
pub const END_EVENT: &str = "End";

/// At most this many loops of one marker fire in a single step, so an absurd
/// speed on a short clip can't flood the scripts.
const MAX_CROSSINGS: i64 = 16;

/// One event crossed this step: its name, how far into the step (`[0, 1)`) and the
/// layer it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct Crossed {
    pub name: String,
    pub at: f32,
    pub layer: usize,
}

/// Sort a step's events into firing order: by when in the step, then layer, then
/// the order they were found in (authored order), a stable sort.
pub fn sort(events: &mut [Crossed]) {
    events.sort_by(|a, b| a.at.total_cmp(&b.at).then(a.layer.cmp(&b.layer)));
}

impl MotionContext<'_> {
    /// The events `playback`'s current motion crossed on `step`. `playhead` is
    /// the motion's rate and wrap.
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
        for (mark, name) in marks {
            for at in crossings(span, mark, bounds) {
                out.push(Crossed {
                    name: name.to_string(),
                    at,
                    layer,
                });
            }
        }
        if let (Bounds::Clamp(end), true) = (bounds, step.end) {
            // Partway through a step that travels onto the end; at its start when
            // the playhead was already there (#1386).
            let at = if span.1 > span.0 {
                ((end - span.0) / (span.1 - span.0)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            out.push(Crossed {
                name: END_EVENT.to_string(),
                at,
                layer,
            });
        }
        out
    }

    /// The current motion's markers in playhead units: a clip node's events in
    /// seconds; a blend tree's dominant child's, normalized by its clip length.
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
                let weights = tree_weights(tree, self.parameters);
                let dominant = weights.iter().enumerate().filter(|&(_, &w)| w > 0.0).fold(
                    None,
                    |best: Option<(usize, f32)>, (i, &w)| match best {
                        Some((_, b)) if b >= w => best,
                        _ => Some((i, w)),
                    },
                );
                let Some((i, _)) = dominant else {
                    return Vec::new();
                };
                let length = self.clip(tree.clips()[i]).map_or(0.0, |c| c.duration);
                if length <= 0.0 {
                    return Vec::new();
                }
                tree.child_events()[i]
                    .iter()
                    .map(|e| (e.time / length, e.name.as_str()))
                    .collect()
            }
        }
    }
}

/// Where in `span` (as a fraction of it, `[0, 1]`) the playhead passes `mark`,
/// repeated every wrap when looping. Forward travel covers `[from, to)`,
/// backward `(to, from]`; a clamped playhead that reaches an end also covers the
/// end itself, since it stops there and the next push against it travels nothing.
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
