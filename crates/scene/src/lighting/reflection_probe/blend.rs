use glam::Vec3;

use super::volume::ProbeFrame;
use super::{ReflectionProbe, ReflectionProbeSet};

/// The sky takes the weight the two probes leave.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeBlend {
    pub first: usize,
    pub first_weight: f32,
    pub second: Option<usize>,
    pub second_weight: f32,
}

impl ReflectionProbe {
    pub fn influence(&self, point: Vec3) -> f32 {
        falloff(self.volume_distance_sq(point), self.blend_distance)
    }
}

/// `outside` is the squared distance past the volume.
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
    pub fn blend(&self, point: Vec3) -> Option<ProbeBlend> {
        pick(&self.probes, point, |i| self.probes[i].influence(point))
    }

    pub fn prepared(&self) -> PreparedProbes {
        let mut prepared = PreparedProbes::default();
        prepared.update(&self.probes);
        prepared
    }
}

/// The one rule both `ReflectionProbeSet::blend` and `PreparedProbes::blend` run, so they
/// agree bit for bit.
fn pick(
    probes: &[ReflectionProbe],
    point: Vec3,
    influence: impl Fn(usize) -> f32,
) -> Option<ProbeBlend> {
    let probe = |i: usize| &probes[i];
    // `Less` when `a` ranks above `b`.
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
    // Equal: proportional, covering as much as the stronger (`w1 >= w2`). Else the first is
    // drawn over the second, Unity's "higher importance wins".
    let (first_weight, second_weight) = match probe(first).importance == probe(second).importance {
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

/// Turns the point into each frame with `influence`'s arithmetic, so it equals
/// `ReflectionProbeSet::blend` bit for bit.
#[derive(Clone, Debug, Default)]
pub struct PreparedProbes {
    probes: Vec<ReflectionProbe>,
    frames: Vec<(ProbeFrame, Option<(Vec3, Vec3)>)>,
}

/// Reach growth relative to the blend distance and the coordinates' size: far past the ulps
/// the reject rounds by, so it stays conservative.
const REACH_SLACK: f32 = 1e-5;

impl PreparedProbes {
    pub fn probes(&self) -> &[ReflectionProbe] {
        &self.probes
    }

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
