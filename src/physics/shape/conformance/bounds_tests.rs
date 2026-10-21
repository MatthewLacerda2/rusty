//! Invariants 1 and 2 (#1171): a collider's world AABB holds the shape physics
//! builds at the same pose, and is tight around it when the turn keeps the axes on
//! the axes. A box drawn round an obliquely turned sphere or cylinder overshoots
//! it by construction, so there it is held to the turned box of the shape's own
//! local box instead: what the corner transform must give.

use glam::Vec3;
use rapier3d::prelude::{Pose as RapierPose, Shape};

use super::{all_samples, turns, world_poses, Sample, SCALES};
use crate::components::{ColliderComponent, Pose};
use crate::physics::shape::build::build_shape;

/// How far two bounds may disagree for a shape whose box spans `extent`: f32
/// rounding of a turn and a world-sized scale, nothing a broadphase would notice.
pub(super) fn tolerance(extent: Vec3) -> f32 {
    1e-5 * extent.max_element() + 1e-6
}

/// The world AABB `calculate_world_aabb` stores for `sample` at `pose`.
fn stored(sample: &Sample, pose: Pose) -> (Vec3, Vec3) {
    let mut c = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    c.shape = sample.shape.clone();
    c.calculate_world_aabb(pose)
}

/// parry's AABB of the shape physics builds for `sample` at `pose`.
fn built(sample: &Sample, pose: Pose) -> (Vec3, Vec3) {
    let collider = build_shape(&sample.shape, pose.scale, sample.geometry())
        .unwrap_or_else(|| panic!("{:?} at {pose:?} builds", sample.shape));
    let aabb = collider
        .shape()
        .compute_aabb(&RapierPose::from_parts(pose.pos, pose.rot));
    (aabb.mins, aabb.maxs)
}

/// The turned box of the shape's own local box: the tightest a world AABB that
/// turns eight corners can be.
fn turned_local_box(sample: &Sample, pose: Pose) -> (Vec3, Vec3) {
    let (a, b) = sample.shape.scaled(pose.scale).local_box();
    let half = (b - a).abs() * 0.5;
    let cuboid = rapier3d::prelude::Cuboid::new(half);
    let centre = pose.pos + pose.rot * ((a + b) * 0.5);
    let aabb = cuboid.compute_aabb(&RapierPose::from_parts(centre, pose.rot));
    (aabb.mins, aabb.maxs)
}

/// Every sample × scale × turn × (root, child), with whether the turn is axial.
fn each(mut check: impl FnMut(&Sample, Pose, bool)) {
    for sample in &all_samples() {
        for scale in SCALES {
            for (turn, axial) in turns() {
                for pose in world_poses(scale, turn) {
                    check(sample, pose, axial);
                }
            }
        }
    }
}

#[test]
fn the_world_aabb_holds_the_built_shape() {
    each(|sample, pose, _| {
        let ((min, max), (want_min, want_max)) = (stored(sample, pose), built(sample, pose));
        let eps = tolerance(want_max - want_min);
        assert!(
            min.cmple(want_min + eps).all() && max.cmpge(want_max - eps).all(),
            "{:?} at {pose:?}: stored {min}..{max} misses built {want_min}..{want_max}",
            sample.shape
        );
    });
}

#[test]
fn the_world_aabb_is_tight() {
    each(|sample, pose, axial| {
        let (min, max) = stored(sample, pose);
        let (want_min, want_max) = if axial {
            built(sample, pose)
        } else {
            turned_local_box(sample, pose)
        };
        let over = (want_min - min).max(max - want_max).max_element();
        assert!(
            over <= tolerance(want_max - want_min),
            "{:?} at {pose:?}: stored {min}..{max} overshoots {want_min}..{want_max} by {over}",
            sample.shape
        );
    });
}
