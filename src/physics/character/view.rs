//! src/physics/character/view.rs — the query view rapier's controller sweeps
//! (#2061): the live colliders, a body table that rides nothing, and exact
//! contact normals. Each stands in for something in rapier 0.36 / parry 0.31
//! that stopped or shortened a walk across flat ground.
//!
//! - **No kinematic bodies** ([`ControllerBodies`]). rapier's controller rides a
//!   kinematic body (`detect_grounded_status_and_apply_friction`), which a
//!   Unity controller does not do: on each slide it resets the remaining
//!   motion's part along that body's contact normal to the body's own velocity,
//!   zero for one at rest. That erases the nudge off the floor rapier adds to
//!   every slide, so on a floor the capsule rests within its skin of, the next
//!   cast reads as moving into it whenever the floor's normal leans a hair
//!   against the walk, hits at distance 0, and the move stalls for good. A
//!   Collider without a Rigidbody is a kinematic body, so that was most floors
//!   in Play. The view's table shows every kinematic body as fixed and keeps
//!   the dynamic ones, which autostep's "never onto, nor through, a dynamic
//!   body" reads.
//! - **Exact normals** ([`ExactNormals`]): a cast's hit takes the closest-points
//!   normal at the pose of impact, not GJK's skewed one.

use std::sync::Arc;

use rapier3d::prelude::*;

#[cfg(doc)]
use super::bodies::ControllerBodies;
use super::normals::ExactNormals;
use crate::physics::world::PhysicsWorld;

/// What the controller's sweeps query through: the controller's body table and
/// the exact-normals dispatcher, built per query ([`PhysicsWorld::controller_view`])
/// since the pipeline only borrows them.
pub(super) struct ControllerView<'a> {
    bodies: Arc<RigidBodySet>,
    exact: ExactNormals<'a>,
    world: &'a PhysicsWorld,
}

impl<'a> ControllerView<'a> {
    /// The live colliders under `filter`, seen through this view.
    pub(super) fn queries(&'a self, filter: QueryFilter<'a>) -> QueryPipeline<'a> {
        let (broad_phase, colliders) = (&self.world.broad_phase, &self.world.colliders);
        broad_phase.as_query_pipeline(&self.exact, &self.bodies, colliders, filter)
    }
}

impl PhysicsWorld {
    /// The view rapier's controller sweeps (see the module doc).
    pub(super) fn controller_view(&self) -> ControllerView<'_> {
        ControllerView {
            bodies: self.controller_bodies.of(&self.bodies),
            exact: ExactNormals(self.narrow_phase.query_dispatcher()),
            world: self,
        }
    }
}
