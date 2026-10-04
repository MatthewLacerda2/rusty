//! src/api/physics/impulse.rs — forces and impulses at a point (#466).
//!
//! Unity's `Rigidbody.AddForceAtPosition` and `AddForceAtPosition(…,
//! ForceMode.Impulse)`: a push at a world point both moves the body and spins it,
//! so the shot that kills an enemy throws its ragdoll the way it came. The
//! velocity change comes from the live body's mass, centre of mass and inertia
//! (`PhysicsWorld::impulse_response`) and lands on the Rigidbody's velocities,
//! like `AddForce`. Outside Play (no physics world) only the linear part applies,
//! `J / mass`. A no-op on a kinematic body or an entity without a Rigidbody.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Table;

use super::super::{put, Reg};
use crate::physics::PhysicsWorld;
use crate::scene::Scene;
use crate::time::FIXED_DELTA_TIME;

/// `(id, x, y, z, px, py, pz)`: a vector and the world point it acts at.
type AtPoint = (u32, f32, f32, f32, f32, f32, f32);

/// Register `AddForceAtPosition` / `AddImpulseAtPosition` onto `Physics`.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    // A continuous force feeds one fixed step, as `AddForce` does: F · dt.
    for (name, scale) in [
        ("AddForceAtPosition", FIXED_DELTA_TIME),
        ("AddImpulseAtPosition", 1.0),
    ] {
        let f = scope.create_function(move |_, (id, x, y, z, px, py, pz): AtPoint| {
            let impulse = Vec3::new(x, y, z) * scale;
            apply(scene, physics, id, impulse, Vec3::new(px, py, pz));
            Ok(())
        });
        put(table, name, f)?;
    }
    Ok(())
}

/// Add the velocity change `impulse` at `point` causes to `id`'s Rigidbody.
fn apply(
    scene: &RefCell<Scene>,
    physics: &RefCell<Option<PhysicsWorld>>,
    id: u32,
    impulse: Vec3,
    point: Vec3,
) {
    let response = physics
        .borrow()
        .as_ref()
        .and_then(|p| p.impulse_response(id, impulse, point));
    let mut scene = scene.borrow_mut();
    let Some(mut rb) = scene.world.rigidbody_mut(id) else {
        return;
    };
    if rb.is_kinematic {
        return;
    }
    let (dv, dw) = response.unwrap_or((impulse / rb.mass.max(0.0001), Vec3::ZERO));
    rb.velocity += dv;
    rb.angular_velocity += dw;
}
