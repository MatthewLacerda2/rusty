//! src/physics/character/normals.rs — the query dispatcher whose shape casts
//! report exact contact normals (#2061). A shape cast that stops just short of
//! the skin takes its normal from GJK's directional distance, which parry
//! itself calls unreliable at small distances but replaces only under 1e-4 m.
//! On a flat box face it leaned up to 6° against the walk, and the slide along
//! it hopped the capsule a few centimetres and cost up to 3% of the stride. A
//! mesh's seams get the mesh's own internal-edge correction ([`seam_corrected`]).

use rapier3d::parry::query::{
    ClosestPoints, Contact, NonlinearRigidMotion, QueryDispatcher, ShapeCastHit, ShapeCastOptions,
    ShapeDistance, ShapeIntersection, Unsupported,
};
use rapier3d::parry::shape::{CompositeShape, Shape};
use rapier3d::prelude::*;

/// How far past the cast's target distance the contact query at the pose of
/// impact looks: the cast stops within GJK's tolerance of the target, so the
/// pair is that close. A pair farther apart than this (a cast that stopped on
/// a penetration it could not resolve, say) keeps the cast's own normal
/// rather than one taken from a pose the cast never reached.
const CONTACT_MARGIN: Real = 0.01;

/// A query dispatcher whose shape casts report the closest-points normal at
/// the time of impact (what parry's own cast falls back on near contact),
/// everything else forwarded unchanged.
pub(super) struct ExactNormals<'a>(pub(super) &'a dyn QueryDispatcher);

impl QueryDispatcher for ExactNormals<'_> {
    fn intersection_test(
        &self,
        pos12: &Pose,
        g1: &dyn Shape,
        g2: &dyn Shape,
    ) -> Result<ShapeIntersection, Unsupported> {
        self.0.intersection_test(pos12, g1, g2)
    }

    fn distance(
        &self,
        pos12: &Pose,
        g1: &dyn Shape,
        g2: &dyn Shape,
    ) -> Result<ShapeDistance, Unsupported> {
        self.0.distance(pos12, g1, g2)
    }

    fn contact(
        &self,
        pos12: &Pose,
        g1: &dyn Shape,
        g2: &dyn Shape,
        prediction: Real,
    ) -> Result<Option<Contact>, Unsupported> {
        self.0.contact(pos12, g1, g2, prediction)
    }

    fn closest_points(
        &self,
        pos12: &Pose,
        g1: &dyn Shape,
        g2: &dyn Shape,
        max_dist: Real,
    ) -> Result<ClosestPoints, Unsupported> {
        self.0.closest_points(pos12, g1, g2, max_dist)
    }

    /// The cast's hit with its normal made exact: the closest-points normal
    /// at the pose of impact. A convex pair (a box, sphere, cylinder, capsule
    /// or hull against the capsule) takes it from the pair. A triangle mesh or
    /// compound takes it from the one part the hit struck, since against the
    /// whole mesh the face under the capsule would hide the edge of the
    /// triangle the walk met; the normal never changes which part's hit is
    /// the earliest, so the cast itself is parry's. A heightfield keeps the
    /// cast's normal (rusty builds none as colliders).
    fn cast_shapes(
        &self,
        pos12: &Pose,
        local_vel12: Vector,
        g1: &dyn Shape,
        g2: &dyn Shape,
        options: ShapeCastOptions,
    ) -> Result<Option<ShapeCastHit>, Unsupported> {
        let Some(mut hit) = self.0.cast_shapes(pos12, local_vel12, g1, g2, options)? else {
            return Ok(None);
        };
        let mut at = *pos12;
        at.translation += local_vel12 * hit.time_of_impact;
        let reach = options.target_distance + CONTACT_MARGIN;
        if g1.as_support_map().is_some() && g2.as_support_map().is_some() {
            // A pair `contact` finds out of reach keeps the cast's own normal.
            if let Ok(Some(contact)) = self.0.contact(&at, g1, g2, reach) {
                hit.normal1 = contact.normal1;
                hit.normal2 = contact.normal2;
            }
        } else if let Some(parts) = g1.as_composite_shape() {
            self.part_corrected(parts, &at, g2, reach, &mut hit);
            seam_corrected(parts, pos12, &mut hit);
        }
        Ok(Some(hit))
    }

    fn cast_shapes_nonlinear(
        &self,
        motion1: &NonlinearRigidMotion,
        g1: &dyn Shape,
        motion2: &NonlinearRigidMotion,
        g2: &dyn Shape,
        start_time: Real,
        end_time: Real,
        stop_at_penetration: bool,
    ) -> Result<Option<ShapeCastHit>, Unsupported> {
        self.0.cast_shapes_nonlinear(
            motion1,
            g1,
            motion2,
            g2,
            start_time,
            end_time,
            stop_at_penetration,
        )
    }
}

impl ExactNormals<'_> {
    /// `hit`'s normal from the part of `parts` it struck, the capsule at `at`
    /// (its pose in the composite's frame). A part `contact` does not support
    /// (a mesh inside a compound), or finds out of reach, keeps the cast's.
    fn part_corrected(
        &self,
        parts: &dyn CompositeShape,
        at: &Pose,
        g2: &dyn Shape,
        reach: Real,
        hit: &mut ShapeCastHit,
    ) {
        parts.map_part_at(hit.subshape1, &mut |pose, part, _| {
            let local = pose.map_or(*at, |p| p.inv_mul(at));
            if let Ok(Some(contact)) = self.0.contact(&local, part, g2, reach) {
                let rot = pose.map(|p| p.rotation).unwrap_or_default();
                hit.normal1 = rot * contact.normal1;
                hit.normal2 = contact.normal2;
            }
        });
    }
}

/// A mesh hit reached ahead of the impact (not at distance 0), its normal
/// corrected by the struck triangle's internal-edge constraint (the mesh's
/// `FIX_INTERNAL_EDGES`, #1880), which parry's cast leaves unapplied: on a
/// seam between two coplanar triangles the edge's normal leans toward the
/// capsule, and the slide along it hopped a fast walk. A hit at distance 0 is
/// left alone: parry decided it by the edge's own normal, and flattening only
/// the reported one meets the same seam again on the next slide, for good.
fn seam_corrected(parts: &dyn CompositeShape, pos12: &Pose, hit: &mut ShapeCastHit) {
    if hit.time_of_impact <= 0.0 {
        return;
    }
    parts.map_part_at(hit.subshape1, &mut |pose, _, constraints| {
        let Some(constraints) = constraints else {
            return;
        };
        let rot = pose.map(|p| p.rotation).unwrap_or_default();
        let mut normal = rot.inverse() * hit.normal1;
        if constraints.project_local_normal_mut(&mut normal) {
            hit.normal1 = rot * normal;
            hit.normal2 = pos12.rotation.inverse() * -hit.normal1;
        }
    });
}
