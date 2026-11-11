//! src/physics/character/normals.rs — the query dispatcher whose shape casts
//! report exact contact normals (#2061). A shape cast that stops just short of
//! the skin takes its normal from GJK's directional distance, which parry
//! itself calls unreliable at small distances but replaces only under 1e-4 m.
//! On a flat box face it leaned up to 6° against the walk, and the slide along
//! it hopped the capsule a few centimetres and cost up to 3% of the stride. A
//! mesh or compound is cast part by part ([`cast_parts`]), so each part's hit
//! gets its exact normal and the mesh's own internal-edge correction.

use rapier3d::parry::query::{
    ClosestPoints, Contact, NonlinearRigidMotion, QueryDispatcher, ShapeCastHit, ShapeCastOptions,
    ShapeDistance, ShapeIntersection, Unsupported,
};
use rapier3d::parry::shape::Shape;
use rapier3d::prelude::*;

use super::parts::cast_parts;

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
    /// compound is cast part by part through this dispatcher ([`cast_parts`]),
    /// so the one part a hit struck gives its normal (against the whole mesh
    /// the face under the capsule would hide the edge of the triangle the walk
    /// met), corrected by the part's internal-edge constraint. A heightfield
    /// keeps the cast's normal (rusty builds none as colliders).
    fn cast_shapes(
        &self,
        pos12: &Pose,
        local_vel12: Vector,
        g1: &dyn Shape,
        g2: &dyn Shape,
        options: ShapeCastOptions,
    ) -> Result<Option<ShapeCastHit>, Unsupported> {
        if let Some(parts) = g1.as_composite_shape() {
            return Ok(cast_parts(self, parts, pos12, local_vel12, g2, options));
        }
        let Some(mut hit) = self.0.cast_shapes(pos12, local_vel12, g1, g2, options)? else {
            return Ok(None);
        };
        if g1.as_support_map().is_some() && g2.as_support_map().is_some() {
            let mut at = *pos12;
            at.translation += local_vel12 * hit.time_of_impact;
            let reach = options.target_distance + CONTACT_MARGIN;
            // A pair `contact` finds out of reach keeps the cast's own normal.
            if let Ok(Some(contact)) = self.0.contact(&at, g1, g2, reach) {
                hit.normal1 = contact.normal1;
                hit.normal2 = contact.normal2;
            }
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
