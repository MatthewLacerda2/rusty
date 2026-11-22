//! rapier resolves two colliders' combine rules the way Unity does (the higher-priority mode wins,
//! `Average < Minimum < Multiply < Maximum`), so the mapping is one-to-one.

use rapier3d::prelude::*;

use super::watch::Owners;
use super::world::{collider_to_write, PhysicsWorld};
use rusty_components::{CombineMode, PhysicsMaterial};
use rusty_scene::physics_material::effective;
use rusty_scene::Scene;

pub(super) fn combine_rule(mode: CombineMode) -> CoefficientCombineRule {
    match mode {
        CombineMode::Average => CoefficientCombineRule::Average,
        CombineMode::Minimum => CoefficientCombineRule::Min,
        CombineMode::Multiply => CoefficientCombineRule::Multiply,
        CombineMode::Maximum => CoefficientCombineRule::Max,
    }
}

pub(super) fn material_differs(collider: &Collider, material: &PhysicsMaterial) -> bool {
    collider.friction() != material.friction
        || collider.restitution() != material.bounciness
        || collider.friction_combine_rule() != combine_rule(material.friction_combine)
        || collider.restitution_combine_rule() != combine_rule(material.bounce_combine)
}

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
    pub(super) fn sync_materials(&mut self, scene: &Scene, owners: &Owners) {
        for (_, ids) in owners.pick(&self.plan) {
            for id in ids {
                let (Some(c), Some(&handle)) =
                    (scene.world.collider(*id), self.id_to_collider.get(id))
                else {
                    continue;
                };
                let material = effective(&c);
                let differs = |held: &Collider| material_differs(held, &material);
                if let Some(collider) = collider_to_write(&mut self.colliders, handle, differs) {
                    apply_material(collider, &material);
                }
            }
        }
    }
}
