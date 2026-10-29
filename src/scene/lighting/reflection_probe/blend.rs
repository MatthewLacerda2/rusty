//! Unity's *Blend Probes* (#1584): the two probes an object between rooms reflects,
//! and how much of each.
//!
//! A probe's **influence** at a point is 1 inside its box and falls linearly to 0
//! at `blend_distance` past it (a zero distance is a hard edge). The candidates are
//! the probes with any influence, ranked by importance, then influence, then the
//! nearer capture point, then the lower index. The two at the top share the
//! reflection:
//!   * **equal importance** — they split it in proportion to their influence, and
//!     together cover as much as the stronger one, so walking through a doorway
//!     from one box into the next fades across the blend distances;
//!   * **lower importance second** — the first is drawn over it: it takes its own
//!     influence, and the second fills what is left, so leaving a small important
//!     box fades into the room around it (Unity's "higher importance wins");
//!   * whatever the probes don't cover reflects the sky, so an object leaving the
//!     last box fades into the sky rather than popping.
//!
//! Per object, on the CPU, at its world-bounds centre, as Unity does; the shader
//! takes two probe indices and their weights. Pure and deterministic.

use glam::Vec3;

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
    /// How strongly this probe applies at `point`: 1 in its box, falling linearly to
    /// 0 at [`Self::blend_distance`] past it.
    pub fn influence(&self, point: Vec3) -> f32 {
        let outside = self.box_distance_sq(point);
        if outside <= 0.0 {
            return 1.0;
        }
        if self.blend_distance <= 0.0 {
            return 0.0;
        }
        (1.0 - outside.sqrt() / self.blend_distance).max(0.0)
    }
}

impl ReflectionProbeSet {
    /// The blend an object centred at `point` reflects (see the module docs), or
    /// `None` when no probe reaches it (it reflects the sky).
    pub fn blend(&self, point: Vec3) -> Option<ProbeBlend> {
        let mut candidates: Vec<(usize, f32)> = (self.probes.iter().enumerate())
            .map(|(i, p)| (i, p.influence(point)))
            .filter(|&(_, w)| w > 0.0)
            .collect();
        let probe = |i: usize| &self.probes[i];
        candidates.sort_by(|&(a, wa), &(b, wb)| {
            (probe(b).importance.cmp(&probe(a).importance))
                .then(wb.total_cmp(&wa))
                .then((probe(a).distance_sq(point)).total_cmp(&probe(b).distance_sq(point)))
                .then(a.cmp(&b))
        });
        let &(first, w1) = candidates.first()?;
        let Some(&(second, w2)) = candidates.get(1) else {
            return Some(ProbeBlend {
                first,
                first_weight: w1,
                second: None,
                second_weight: 0.0,
            });
        };
        let (first_weight, second_weight) =
            match probe(first).importance == probe(second).importance {
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
}

#[cfg(test)]
#[path = "blend_tests.rs"]
mod blend_tests;
