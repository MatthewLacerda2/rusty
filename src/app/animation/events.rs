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
//! - plus the automatic [`END_EVENT`] when a non-looping motion reaches its end.
//!
//! Every step's events are ordered by when in the step they were crossed, then
//! by layer, then authored order — deterministic, and true time order.

use crate::components::{Motion, Playback, Playhead};

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
    /// The events `playback`'s current motion crossed travelling `span` this
    /// step. `playhead` is the motion's rate and wrap.
    pub fn crossed(
        &self,
        playback: &Playback,
        playhead: Playhead,
        span: (f32, f32),
        layer: usize,
    ) -> Vec<Crossed> {
        let wrap = (playback.loop_clip && playhead.wrap > 0.0).then_some(playhead.wrap);
        let mut marks: Vec<(f32, &str)> = self.marks(playback);
        if wrap.is_none() && playhead.wrap > 0.0 {
            marks.push((playhead.wrap, END_EVENT));
        }
        let mut out = Vec::new();
        for (mark, name) in marks {
            for at in crossings(span, mark, wrap) {
                out.push(Crossed {
                    name: name.to_string(),
                    at,
                    layer,
                });
            }
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

/// Where in `span` (as a fraction of it, `[0, 1)`) the playhead passes `mark`,
/// repeated every `wrap` when looping. Forward travel covers `[from, to)`,
/// backward `(to, from]`.
fn crossings((from, to): (f32, f32), mark: f32, wrap: Option<f32>) -> Vec<f32> {
    let span = to - from;
    if span == 0.0 || !span.is_finite() {
        return Vec::new();
    }
    let (first, last) = match (wrap, span > 0.0) {
        (None, true) => (0, i64::from(from <= mark && mark < to) - 1),
        (None, false) => (0, i64::from(to < mark && mark <= from) - 1),
        (Some(w), true) => (
            ((from - mark) / w).ceil() as i64,
            ((to - mark) / w).ceil() as i64 - 1,
        ),
        (Some(w), false) => (
            ((to - mark) / w).floor() as i64 + 1,
            ((from - mark) / w).floor() as i64,
        ),
    };
    let last = last.min(first + MAX_CROSSINGS - 1);
    (first..=last)
        .map(|k| {
            let at = mark + k as f32 * wrap.unwrap_or(0.0);
            ((at - from) / span).clamp(0.0, 1.0)
        })
        .collect()
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod events_tests;
