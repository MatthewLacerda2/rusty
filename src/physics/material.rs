//! src/physics/material.rs — the collider physics material on rapier (#447).
//!
//! Maps the engine's `PhysicsMaterial` onto a rapier collider's friction,
//! restitution and their combine rules. rapier resolves two colliders' rules the
//! way Unity does (the higher-priority mode wins, `Average < Minimum < Multiply <
//! Maximum`), so the mapping is one-to-one and the solver does the combining.
//! Applied at collider build and re-applied whenever the Collider changes, so a material edited
//! mid-play (`Physics.SetPhysicsMaterial`) takes effect on the next step.

use rapier3d::prelude::*;

use super::watch::Owners;
use super::world::{collider_to_write, PhysicsWorld};
use crate::components::{CombineMode, PhysicsMaterial};
use crate::scene::Scene;

/// rapier's rule for a combine mode.
pub(super) fn combine_rule(mode: CombineMode) -> CoefficientCombineRule {
    match mode {
        CombineMode::Average => CoefficientCombineRule::Average,
        CombineMode::Minimum => CoefficientCombineRule::Min,
        CombineMode::Multiply => CoefficientCombineRule::Multiply,
        CombineMode::Maximum => CoefficientCombineRule::Max,
    }
}

/// Whether `collider` holds anything but `material`.
pub(super) fn material_differs(collider: &Collider, material: &PhysicsMaterial) -> bool {
    collider.friction() != material.friction
        || collider.restitution() != material.bounciness
        || collider.friction_combine_rule() != combine_rule(material.friction_combine)
        || collider.restitution_combine_rule() != combine_rule(material.bounce_combine)
}

/// Write `material` onto `collider`, touching only what differs so an
/// unchanged collider is left alone.
pub(super) fn apply_material(collider: &mut Collider, material: &PhysicsMaterial) {
    if collider.friction() != material.friction {
        collider.set_friction(material.friction);
    }
    if collider.restitution() != material.bounciness {
        collider.set_restitution(material.bounciness);
    }
    let friction_rule = combine_rule(material.friction_combine);
    if collider.friction_combine_rule() != friction_rule {
        collider.set_friction_combine_rule(friction_rule);
    }
    let bounce_rule = combine_rule(material.bounce_combine);
    if collider.restitution_combine_rule() != bounce_rule {
        collider.set_restitution_combine_rule(bounce_rule);
    }
}

impl PhysicsWorld {
    /// Re-apply each built collider's material from its component, so a
    /// material edited since the last tick reaches the solver.
    pub(super) fn sync_materials(&mut self, scene: &Scene, owners: &Owners) {
        for (_, ids) in owners.pick(&self.plan) {
            for id in ids {
                let (Some(c), Some(&handle)) =
                    (scene.world.collider(*id), self.id_to_collider.get(id))
                else {
                    continue;
                };
                let differs = |held: &Collider| material_differs(held, &c.material);
                if let Some(collider) = collider_to_write(&mut self.colliders, handle, differs) {
                    apply_material(collider, &c.material);
                }
            }
        }
    }
}
