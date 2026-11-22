use rapier3d::parry::query::{
    ClosestPoints, Contact, NonlinearRigidMotion, QueryDispatcher, ShapeCastHit, ShapeCastOptions,
    ShapeDistance, ShapeIntersection, Unsupported,
};
use rapier3d::parry::shape::Shape;
use rapier3d::prelude::*;

use super::cost::{self, Work};
use super::parts::cast_parts;

/// Metres past the cast's target distance the contact query looks: the cast stops within
/// GJK's tolerance of it. A pair farther apart keeps the cast's own normal, not one taken
/// from a pose the cast never reached.
const CONTACT_MARGIN: Real = 0.01;

pub(super) fn exact_normal(
    dispatcher: &dyn QueryDispatcher,
    mut hit: ShapeCastHit,
    pos12: &Pose,
    vel12: Vector,
    [g1, g2]: [&dyn Shape; 2],
    options: ShapeCastOptions,
) -> ShapeCastHit {
    let mut at = *pos12;
    at.translation += vel12 * hit.time_of_impact;
    let reach = options.target_distance + CONTACT_MARGIN;
    cost::count(Work::Contact);
    if let Ok(Some(contact)) = dispatcher.contact(&at, g1, g2, reach) {
        hit.normal1 = contact.normal1;
        hit.normal2 = contact.normal2;
    }
    hit
}

/// Casts reporting the closest-points normal at impact: GJK's directional normal, which parry
/// replaces only under 1e-4 m, leaned up to 6° on a flat face and hopped the capsule.
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

    /// A mesh or compound is cast part by part: against the whole mesh the face under the capsule
    /// would hide the edge of the triangle the walk met.
    fn cast_shapes(
        &self,
        pos12: &Pose,
        local_vel12: Vector,
        g1: &dyn Shape,
        g2: &dyn Shape,
        options: ShapeCastOptions,
    ) -> Result<Option<ShapeCastHit>, Unsupported> {
        if g1.as_composite_shape().is_some() {
            return Ok(cast_parts(self.0, g1, pos12, local_vel12, g2, options));
        }
        let Some(hit) = self.0.cast_shapes(pos12, local_vel12, g1, g2, options)? else {
            return Ok(None);
        };
        if g1.as_support_map().is_some() && g2.as_support_map().is_some() {
            return Ok(Some(exact_normal(
                self.0,
                hit,
                pos12,
                local_vel12,
                [g1, g2],
                options,
            )));
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
