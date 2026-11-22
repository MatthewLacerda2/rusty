use std::sync::Arc;

use rapier3d::prelude::*;

#[cfg(doc)]
use super::bodies::ControllerBodies;
use super::normals::ExactNormals;
use crate::world::PhysicsWorld;

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
