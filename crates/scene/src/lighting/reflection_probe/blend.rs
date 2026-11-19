//! Unity's *Blend Probes* (#1584): the two probes an object between rooms reflects,
//! and how much of each.
//!
//! A probe's **influence** at a point is 1 inside its volume (a turned box or a
//! sphere, #1995) and falls linearly to 0
//! at `blend_distance` past it (a zero distance is a hard edge). The candidates are
//! the probes with any influence, ranked by importance, then influence, then the
//! nearer capture point, then the lower index. The two at the top share the
//! reflection:
//!   * **equal importance** — they split it in proportion to their influence, and
//!     together cover as much as the stronger one, so walking through a doorway
//!     from one volume into the next fades across the blend distances;
//!   * **lower importance second** — the first is drawn over it: it takes its own
//!     influence, and the second fills what is left, so leaving a small important
//!     volume fades into the room around it (Unity's "higher importance wins");
//!   * whatever the probes don't cover reflects the sky, so an object leaving the
//!     last volume fades into the sky rather than popping.
//!
//! Per object, on the CPU, at its world-bounds centre, as Unity does; the shader
//! takes two probe indices and their weights. Pure and deterministic.
//!
//! The renderer asks once per drawn object per camera, so it asks a
//! [`PreparedProbes`]: the same rule over frames built once per resident set, with a
//! probe whose reach misses the point skipped before its influence is measured.

use glam::Vec3;

use super::volume::ProbeFrame;
use super::{ReflectionProbe, ReflectionProbeSet};

/// The probes one object reflects: `first` with weight `first_weight`, `second`
/// (if any) with `second_weight`, and the sky with what is left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeBlend {
    pub first: usize,
    pub first_weight: f32,
    pub second: Option<usize>,
    pub second_weight: f32,
}

impl ReflectionProbe {
    /// How strongly this probe applies at `point`: 1 in its volume, falling linearly to
    /// 0 at [`Self::blend_distance`] past it.
    pub fn influence(&self, point: Vec3) -> f32 {
        falloff(self.volume_distance_sq(point), self.blend_distance)
    }
}

/// The influence of a probe whose volume lies `outside` (squared) from the point.
fn falloff(outside: f32, blend_distance: f32) -> f32 {
    if outside <= 0.0 {
        return 1.0;
    }
    if blend_distance <= 0.0 {
        return 0.0;
    }
    (1.0 - outside.sqrt() / blend_distance).max(0.0)
}

impl ReflectionProbeSet {
    /// The blend an object centred at `point` reflects (see the module docs), or
    /// `None` when no probe reaches it (it reflects the sky). One pass keeping the
    /// top two, no allocation.
    pub fn blend(&self, point: Vec3) -> Option<ProbeBlend> {
        pick(&self.probes, point, |i| self.probes[i].influence(point))
    }

    /// The set prepared for many [`PreparedProbes::blend`] calls.
    pub fn prepared(&self) -> PreparedProbes {
        let mut prepared = PreparedProbes::default();
        prepared.update(&self.probes);
        prepared
    }
}

/// The blend of `probes` at `point`, given each probe's influence there (0 for one
/// that misses it): the one rule both [`ReflectionProbeSet::blend`] and
/// [`PreparedProbes::blend`] run, so they agree bit for bit.
fn pick(
    probes: &[ReflectionProbe],
    point: Vec3,
    influence: impl Fn(usize) -> f32,
) -> Option<ProbeBlend> {
    let probe = |i: usize| &probes[i];
    // `Less` when candidate `a` ranks above `b`.
    let rank = |&(a, wa): &(usize, f32), &(b, wb): &(usize, f32)| {
        (probe(b).importance.cmp(&probe(a).importance))
            .then(wb.total_cmp(&wa))
            .then((probe(a).distance_sq(point)).total_cmp(&probe(b).distance_sq(point)))
            .then(a.cmp(&b))
    };
    let (mut top, mut next) = (None::<(usize, f32)>, None::<(usize, f32)>);
    for i in 0..probes.len() {
        let c = (i, influence(i));
        if c.1 <= 0.0 {
            continue;
        }
        if top.is_none_or(|t| rank(&c, &t).is_lt()) {
            (top, next) = (Some(c), top);
        } else if next.is_none_or(|n| rank(&c, &n).is_lt()) {
            next = Some(c);
        }
    }
    let (first, w1) = top?;
    let Some((second, w2)) = next else {
        return Some(ProbeBlend {
            first,
            first_weight: w1,
            second: None,
            second_weight: 0.0,
        });
    };
    let (first_weight, second_weight) = match probe(first).importance == probe(second).importance {
        // Proportional, covering as much as the stronger one (`w1 >= w2`).
        true => (w1 * w1 / (w1 + w2), w1 * w2 / (w1 + w2)),
        false => (w1, (1.0 - w1) * w2),
    };
    Some(ProbeBlend {
        first,
        first_weight,
        second: Some(second),
        second_weight,
    })
}

/// A set of probes ready for many blends: each probe's [`ProbeFrame`], built once
/// when the probe changes rather than on every influence, and its *reach*, the
/// frame-space box its influence cannot leave. [`Self::blend`] equals
/// [`ReflectionProbeSet::blend`] over [`Self::probes`] bit for bit: the point is
/// turned into each frame with the arithmetic `influence` uses, a probe whose reach
/// misses the turned point is skipped, where its influence would read 0, and every
/// other one is measured from that same turned point. The probes are read-only, so
/// no frame goes stale; [`Self::update`] is the one way to change them.
#[derive(Clone, Debug, Default)]
pub struct PreparedProbes {
    probes: Vec<ReflectionProbe>,
    frames: Vec<(ProbeFrame, Option<(Vec3, Vec3)>)>,
}

/// How much a probe's reach is grown past its blend distance, relative to that
/// distance and to the size of the frame's coordinates: far past the few ulps by
/// which the nearest point, its distance and the square root round, so the reject
/// stays conservative.
const REACH_SLACK: f32 = 1e-5;

impl PreparedProbes {
    /// The probes, in the order the blend's indices name them.
    pub fn probes(&self) -> &[ReflectionProbe] {
        &self.probes
    }

    /// Make the set `probes`, in order, preparing only the probes that differ from
    /// the one held at their index.
    pub fn update<'a>(&mut self, probes: impl IntoIterator<Item = &'a ReflectionProbe>) {
        let mut n = 0;
        for (i, probe) in probes.into_iter().enumerate() {
            n = i + 1;
            match self.probes.get_mut(i) {
                Some(held) if held == probe => continue,
                Some(held) => held.clone_from(probe),
                None => self.probes.push(probe.clone()),
            }
            let frame = probe.frame();
            let prepared = (frame, reach(&frame));
            match self.frames.get_mut(i) {
                Some(slot) => *slot = prepared,
                None => self.frames.push(prepared),
            }
        }
        self.probes.truncate(n);
        self.frames.truncate(n);
    }

    /// [`ReflectionProbeSet::blend`] of [`Self::probes`] at `point`.
    pub fn blend(&self, point: Vec3) -> Option<ProbeBlend> {
        pick(&self.probes, point, |i| {
            let (frame, reach) = &self.frames[i];
            let local = frame.to_frame(point);
            if reach.is_some_and(|(lo, hi)| !(local.cmpge(lo) & local.cmple(hi)).all()) {
                return 0.0;
            }
            falloff(
                frame.local_distance_sq(local),
                self.probes[i].blend_distance,
            )
        })
    }
}

/// The box, in `frame`'s own space, outside which the probe has no influence: the
/// volume's box (the sphere's bounding cube) grown by the blend distance and
/// [`REACH_SLACK`]. The test runs on the point already turned into the frame, so it
/// holds for any stored rotation, unit or not. `None` (every point is measured)
/// when it is not finite.
fn reach(frame: &ProbeFrame) -> Option<(Vec3, Vec3)> {
    let (lo, hi) = match frame.sphere {
        Some(r) => (frame.center - r, frame.center + r),
        None => (frame.box_min, frame.box_max),
    };
    let scale = lo.abs().max(hi.abs()).max_element();
    let grow = frame.influence * (1.0 + REACH_SLACK) + REACH_SLACK * (1.0 + scale);
    let (lo, hi) = (lo - grow, hi + grow);
    (lo.is_finite() && hi.is_finite()).then_some((lo, hi))
}

#[cfg(test)]
#[path = "blend_tests.rs"]
mod blend_tests;

#[cfg(test)]
#[path = "prepared_tests.rs"]
mod prepared_tests;
