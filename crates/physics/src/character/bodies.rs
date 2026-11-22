use std::sync::{Arc, Mutex, PoisonError};

use rapier3d::prelude::*;

use crate::world::PhysicsWorld;

/// The body table's change counter, and the cached copy with the count it was
/// made at.
#[derive(Default)]
pub(crate) struct ControllerBodies {
    revision: u64,
    cached: Mutex<Option<(u64, Arc<RigidBodySet>)>>,
}

impl ControllerBodies {
    /// The controller's copy of `bodies`, remade if a body was inserted,
    /// removed or retyped since the last one.
    pub(super) fn of(&self, bodies: &RigidBodySet) -> Arc<RigidBodySet> {
        let mut cached = self.cached.lock().unwrap_or_else(PoisonError::into_inner);
        match cached.as_ref() {
            Some((at, copy)) if *at == self.revision => Arc::clone(copy),
            _ => {
                let copy = Arc::new(without_kinematics(bodies));
                *cached = Some((self.revision, Arc::clone(&copy)));
                copy
            }
        }
    }

    /// Switch `body` to `class` (waking it), counting the change. `body` is one
    /// of the table's: the caller borrowed it from the world's `bodies`.
    pub(crate) fn retype(&mut self, body: &mut RigidBody, class: RigidBodyType) {
        self.revision += 1;
        // lint: allow(body-table) the one retype: the revision moved above
        body.set_body_type(class, true);
    }
}

impl PhysicsWorld {
    pub(crate) fn insert_body(&mut self, body: RigidBody) -> RigidBodyHandle {
        self.controller_bodies.revision += 1;
        // lint: allow(body-table) the one insert: the revision moved above
        self.bodies.insert(body)
    }

    /// Remove `handle`'s body and every collider and joint attached to it,
    /// counting the change.
    pub(crate) fn remove_body(&mut self, handle: RigidBodyHandle) {
        self.controller_bodies.revision += 1;
        // lint: allow(body-table) the one remove: the revision moved above
        self.bodies.remove(
            handle,
            &mut self.islands,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.soft_bodies,
            true,
        );
    }
}

/// Kinematic bodies made fixed: rapier's controller rides one (a Collider alone is
/// one), which stalled walks; Unity's rides nothing. Dynamic ones keep their type for
/// autostep.
fn without_kinematics(bodies: &RigidBodySet) -> RigidBodySet {
    let mut copy = bodies.clone();
    for (_, body) in copy.iter_mut() {
        if body.is_kinematic() {
            // lint: allow(body-table) the controller's own copy, not the world's table
            body.set_body_type(RigidBodyType::Fixed, false);
        }
    }
    copy
}

#[cfg(test)]
#[path = "bodies_tests.rs"]
mod tests;
