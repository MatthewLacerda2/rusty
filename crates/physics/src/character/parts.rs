//! crates/physics/src/character/parts.rs — the controller's shape cast against a
//! triangle mesh or compound, part by part, with each part's normal
//! constraints applied inside the cast (#2075). parry's own composite cast
//! (`CompositeShapeRef::cast_shape`) drops the constraints its parts carry, so
//! a mesh's `FIX_INTERNAL_EDGES` (#1880) never reached a sweep: at a seam
//! between two coplanar triangles the capsule, resting within its skin of the
//! floor, met the next triangle's edge at distance 0, its closest-points normal
//! leaning back toward the capsule, and parry counted that as moving into it.
//! The walk lost part of a stride, and a fast one hopped off the floor.

use rapier3d::parry::bounding_volume::Aabb;
use rapier3d::parry::partitioning::BvhNode;
use rapier3d::parry::query::details::NormalConstraints;
use rapier3d::parry::query::{QueryDispatcher, Ray, RayCast, ShapeCastHit, ShapeCastOptions};
use rapier3d::parry::shape::{CompositeShape, Shape};
use rapier3d::prelude::*;

/// Under this time of impact parry decides a support-map cast by the contact
/// normal at the start pose (`cast_shapes_support_map_support_map`'s fallback,
/// taken with `compute_impact_geometry_on_penetration`, which rapier's
/// controller always sets): a pair moving along or away from it is no hit.
const NEAR_TOI: Real = 1.0e-4;

/// The earliest hit of `g2`, posed at `pos12` in `parts`' frame and moving by
/// `vel12`, against any part of `parts`: parry's composite traversal, each
/// part cast through `dispatcher` and then held to its normal constraints
/// ([`constrained`]). The hit's `subshape1` is the struck part, as parry's.
pub(super) fn cast_parts(
    dispatcher: &dyn QueryDispatcher,
    parts: &dyn CompositeShape,
    pos12: &Pose,
    vel12: Vector,
    g2: &dyn Shape,
    options: ShapeCastOptions,
) -> Option<ShapeCastHit> {
    let aabb2 = g2.compute_aabb(pos12);
    let ray = Ray::new(Vector::ZERO, vel12);
    let shift = -aabb2.center();
    let margin = aabb2.half_extents() + Vector::splat(options.target_distance);
    let node_toi = |node: &BvhNode, best: Real| {
        let msum = Aabb {
            mins: node.mins() + shift - margin,
            maxs: node.maxs() + shift + margin,
        };
        msum.cast_local_ray(&ray, best, true).unwrap_or(Real::MAX)
    };
    let part_hit = |id: u32, best: Real| {
        // A part can win only by a hit before the best so far.
        let options = ShapeCastOptions {
            max_time_of_impact: best,
            ..options
        };
        let mut found = None;
        parts.map_part_at(id, &mut |pose, part, constraints| {
            let local = pose.map_or(*pos12, |p| p.inv_mul(pos12));
            let vel = pose.map_or(vel12, |p| p.rotation.inverse() * vel12);
            let hit = dispatcher.cast_shapes(&local, vel, part, g2, options);
            let hit = hit.ok().flatten();
            let hit = hit.and_then(|h| constrained(h, constraints, &local, vel, options));
            found = hit.map(|h| pose.map_or(h, |p| h.transform1_by(p)));
        });
        found
    };
    let best = parts
        .bvh()
        .find_best(options.max_time_of_impact, node_toi, part_hit);
    best.map(|(id, mut hit)| {
        hit.subshape1 = id;
        hit
    })
}

/// A part's `hit` with its normal corrected by the part's `constraints` (none,
/// or a correction they refuse, leave it as cast), or `None` when it no longer
/// stands: a hit within [`NEAR_TOI`] is judged again the way parry judged it,
/// by the corrected normal against `vel`, so a capsule moving along a seam's
/// flat face goes on instead of stopping on the seam's edge. A cast that
/// stops at penetration (`ground_below`'s probe) keeps it, as parry does. Correcting only
/// the reported normal and keeping the hit met the same edge on the next slide
/// and stalled the walk for good (#2073).
fn constrained(
    mut hit: ShapeCastHit,
    constraints: Option<&dyn NormalConstraints>,
    pos12: &Pose,
    vel: Vector,
    options: ShapeCastOptions,
) -> Option<ShapeCastHit> {
    let Some(constraints) = constraints else {
        return Some(hit);
    };
    let mut normal = hit.normal1;
    if !constraints.project_local_normal_mut(&mut normal) {
        return Some(hit);
    }
    let rejudged = !options.stop_at_penetration && hit.time_of_impact < NEAR_TOI;
    if rejudged && normal.dot(vel) >= 0.0 {
        return None;
    }
    hit.normal1 = normal;
    hit.normal2 = pos12.rotation.inverse() * -normal;
    Some(hit)
}

#[cfg(test)]
#[path = "parts_tests.rs"]
mod tests;
