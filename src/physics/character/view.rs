//! src/physics/character/view.rs — the query view rapier's controller sweeps
//! (#2061): the live colliders, with no rigid-body table and exact contact
//! normals. Each stands in for something in rapier 0.36 / parry 0.31 that
//! stopped or shortened a walk across flat ground.
//!
//! - **No body table.** rapier's controller reads it for one thing a Unity
//!   controller does not do, riding a kinematic body
//!   (`detect_grounded_status_and_apply_friction`): on each slide it resets the
//!   remaining motion's part along that body's contact normal to the body's own
//!   velocity, zero for one at rest. That erases the nudge off the floor rapier
//!   adds to every slide, so on a floor the capsule rests within its skin of,
//!   the next cast reads as moving into it whenever the floor's normal leans a
//!   hair against the walk, hits at distance 0, and the move stalls for good. A
//!   Collider without a Rigidbody is a kinematic body, so that was most floors
//!   in Play. The table's other use, autostep's "never onto a dynamic body", is
//!   [`PhysicsWorld::climbed_dynamic`]'s; the filter never reads it (the
//!   character's own body is excluded by handle, liveness by `is_live`'s own
//!   borrow).
//! - **Exact normals** ([`ExactNormals`]). A shape cast that stops just short of
//!   the skin takes its normal from GJK's directional distance, which parry
//!   itself calls unreliable at small distances but replaces only under 1e-4 m.
//!   On a flat box face it leaned up to 6° against the walk, and the slide along
//!   it hopped the capsule a few centimetres and cost up to 3% of the stride.
//!   A mesh's seams get the mesh's own internal-edge correction
//!   ([`seam_corrected`]).

use glam::Vec3;
use rapier3d::parry::query::details::{
    cast_shapes_composite_shape_shape, cast_shapes_heightfield_shape,
};
use rapier3d::parry::query::{
    ClosestPoints, Contact, NonlinearRigidMotion, QueryDispatcher, ShapeCastHit, ShapeCastOptions,
    ShapeDistance, ShapeIntersection, Unsupported,
};
use rapier3d::parry::shape::{CompositeShape, Shape};
use rapier3d::prelude::*;

use super::pass::Pass;
use super::sweep::MoveCtx;
use crate::physics::world::PhysicsWorld;

/// How far past the cast's target distance the contact query at the pose of
/// impact looks: the cast stops within GJK's tolerance of the target, and a
/// bounded reach keeps a triangle mesh's query to the triangles near the hit.
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

    /// The cast's hit with its normal made exact. A convex pair (a box,
    /// sphere, cylinder, capsule or hull against the capsule) takes the
    /// closest-points normal at the pose of impact. A triangle mesh, compound
    /// or heightfield is cast part by part through this dispatcher, so each
    /// part is corrected against itself: against the whole mesh, the face
    /// under the capsule would hide the edge of the triangle the walk met.
    fn cast_shapes(
        &self,
        pos12: &Pose,
        local_vel12: Vector,
        g1: &dyn Shape,
        g2: &dyn Shape,
        options: ShapeCastOptions,
    ) -> Result<Option<ShapeCastHit>, Unsupported> {
        let convex = g1.as_support_map().is_some() && g2.as_support_map().is_some();
        if !convex {
            if let Some(parts) = g1.as_composite_shape() {
                let cast = cast_shapes_composite_shape_shape;
                let mut hit = cast(self, pos12, local_vel12, parts, g2, options);
                if let Some(hit) = hit.as_mut() {
                    seam_corrected(parts, pos12, hit);
                }
                return Ok(hit);
            }
            if let Some(field) = g1.as_heightfield() {
                return cast_shapes_heightfield_shape(self, pos12, local_vel12, field, g2, options);
            }
        }
        let Some(mut hit) = self.0.cast_shapes(pos12, local_vel12, g1, g2, options)? else {
            return Ok(None);
        };
        let mut at = *pos12;
        at.translation += local_vel12 * hit.time_of_impact;
        let reach = options.target_distance + CONTACT_MARGIN;
        // A pair `contact` does not support, or finds out of reach, keeps the
        // cast's own normal.
        if let (true, Ok(Some(contact))) = (convex, self.0.contact(&at, g1, g2, reach)) {
            hit.normal1 = contact.normal1;
            hit.normal2 = contact.normal2;
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

/// What the controller's sweeps query through: an empty body table and the
/// exact-normals dispatcher, built per query ([`PhysicsWorld::controller_view`])
/// since the pipeline only borrows them.
pub(super) struct ControllerView<'a> {
    no_bodies: RigidBodySet,
    exact: ExactNormals<'a>,
    world: &'a PhysicsWorld,
}

impl<'a> ControllerView<'a> {
    /// The live colliders under `filter`, seen through this view.
    pub(super) fn queries(&'a self, filter: QueryFilter<'a>) -> QueryPipeline<'a> {
        let (broad_phase, colliders) = (&self.world.broad_phase, &self.world.colliders);
        broad_phase.as_query_pipeline(&self.exact, &self.no_bodies, colliders, filter)
    }
}

impl PhysicsWorld {
    /// The view rapier's controller sweeps (see the module doc).
    pub(super) fn controller_view(&self) -> ControllerView<'_> {
        ControllerView {
            no_bodies: RigidBodySet::new(),
            exact: ExactNormals(self.narrow_phase.query_dispatcher()),
            world: self,
        }
    }

    /// Whether a pass rose by climbing a dynamic body's side: autostep onto a
    /// crate, which the controller never does (`include_dynamic_bodies: false`,
    /// a rule rapier can't apply without the body table).
    pub(super) fn climbed_dynamic(&self, m: &MoveCtx, motion: Vec3, pass: &Pass) -> bool {
        let rose = pass.translation.y > motion.y.max(0.0) + 1e-5;
        let dynamic = |h: ColliderHandle| {
            let body = self.colliders.get(h).and_then(|c| c.parent());
            body.and_then(|b| self.bodies.get(b))
                .is_some_and(|b| b.is_dynamic())
        };
        let climbed = |c: &rapier3d::control::CharacterCollision| {
            !m.walkable(c.hit.normal1) && dynamic(c.handle)
        };
        rose && pass.collisions.iter().any(climbed)
    }
}
