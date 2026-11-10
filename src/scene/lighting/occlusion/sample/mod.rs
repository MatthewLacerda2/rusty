//! What one cell of the occlusion bake sees (#1714): the static scene ready to
//! trace (`baking`), the points a ray is aimed at (`targets`), and the proofs that
//! spare most rays: one occluder face (`shaft`), the faces on one plane fused
//! (`sheets`), the faces each sampling cube last met (`eye`), tried by object,
//! cluster (`clusters`) and point (`sight`).

mod baking;
mod clusters;
mod eye;
mod shaft;
mod sheets;
mod sight;
mod targets;

pub(super) use baking::Baking;
#[cfg(all(test, feature = "dev"))]
pub(super) use sheets::without_union;
#[cfg(test)]
pub(super) use targets::target_points;
pub use targets::{MAX_TARGETS, TARGET_SPACING, VOXEL};

use glam::Vec3;

/// How far, metres, both boxes must clear an occluder's plane, and a crossing its
/// edges, for a proof to hold: one figure for the polygon's proof (`shaft`) and
/// the sheet's (`sheets`), so the two never disagree on what clears a plane.
const CLEAR: f32 = 1e-3;

/// A box's corners, one when they meet.
fn corners((min, max): (Vec3, Vec3)) -> ([Vec3; 8], usize) {
    let pick = |k: usize, bit: usize, axis: usize| if k & bit == 0 { min[axis] } else { max[axis] };
    let all = std::array::from_fn(|k| Vec3::new(pick(k, 1, 0), pick(k, 2, 1), pick(k, 4, 2)));
    (all, if min == max { 1 } else { 8 })
}

/// Whether boxes `from` and `to` clear the slab `lo <= normal · p <= hi` (a plane
/// when `lo == hi`) by [`CLEAR`], on opposite sides.
fn opposite(normal: Vec3, (lo, hi): (f32, f32), from: (Vec3, Vec3), to: (Vec3, Vec3)) -> bool {
    let centre = |b: (Vec3, Vec3)| normal.dot((b.0 + b.1) * 0.5);
    let reach = |b: (Vec3, Vec3)| normal.abs().dot((b.1 - b.0) * 0.5);
    let (cf, ct) = (centre(from), centre(to));
    let below = cf + reach(from) < lo - CLEAR && ct - reach(to) > hi + CLEAR;
    let above = cf - reach(from) > hi + CLEAR && ct + reach(to) < lo - CLEAR;
    below || above
}

#[cfg(test)]
mod sheets_tests;
#[cfg(test)]
mod union_tests;
