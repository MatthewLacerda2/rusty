//! src/physics/character/body.rs — the CharacterController's capsule as the
//! entity's collider in the physics world: what it is built from, and keeping it
//! sized and upright each tick.

use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use crate::components::{CapsuleAxis, ColliderShape, PhysicsMaterial};
use crate::ecs::World;
use crate::physics::build::{capsule_dims, ColliderInputs};
use crate::physics::compound::{relative_pose, world_pose};
use crate::physics::convert::to_pose;
use crate::physics::world::PhysicsWorld;
use crate::scene::Scene;

/// A CharacterController's collider inputs: its capsule, upright at its centre.
/// `None` without one.
pub(in crate::physics) fn character_collider_inputs(
    world: &World,
    id: u32,
) -> Option<ColliderInputs> {
    let cc = world.character_controller(id)?;
    Some(ColliderInputs {
        shape: ColliderShape::Capsule {
            radius: cc.radius,
            height: cc.height,
            axis: CapsuleAxis::Y,
        },
        center: cc.center,
        upright: true,
        mesh_geom: None,
        is_trigger: false,
        material: PhysicsMaterial::default(),
        layer: world.layer(id),
    })
}

/// The offset of an upright shape on its owner's body: its centre sits at
/// `center` (local to its entity, scaled) and it stays unrotated in the world,
/// so its rotation on the body undoes the owner's.
pub(in crate::physics) fn upright_offset(
    owner_rot: Quat,
    offset_pos: Vec3,
    offset_rot: Quat,
    center: Vec3,
) -> Pose {
    to_pose(offset_pos + offset_rot * center, owner_rot.inverse())
}

impl PhysicsWorld {
    /// Keep each CharacterController's collider the capsule its component
    /// describes: resized when its height or radius changed (a crouch), and
    /// upright at its centre however the entity has turned since the last tick.
    pub(in crate::physics) fn sync_characters(&mut self, scene: &Scene) {
        for (&owner, ids) in &self.plan {
            for &id in ids {
                let Some(cc) = scene.world.character_controller(id).map(|c| c.clone()) else {
                    continue;
                };
                let (Some(owner_pose), Some(pose)) =
                    (world_pose(scene, owner), world_pose(scene, id))
                else {
                    continue;
                };
                let handle = self.id_to_collider.get(&id);
                let Some(collider) = handle.and_then(|&h| self.colliders.get_mut(h)) else {
                    continue;
                };
                let (pos, rot) = relative_pose(&owner_pose, &pose);
                let offset = upright_offset(owner_pose.rot, pos, rot, cc.center * pose.scale);
                if collider.position_wrt_parent() != Some(&offset) {
                    collider.set_position_wrt_parent(offset);
                }
                let (half_segment, radius) =
                    capsule_dims(cc.radius, cc.height, CapsuleAxis::Y, pose.scale);
                let same = collider.shape().as_capsule().is_some_and(|c| {
                    (c.half_height() - half_segment).abs() < 1e-6
                        && (c.radius - radius).abs() < 1e-6
                });
                if !same {
                    collider.set_shape(SharedShape::capsule_y(half_segment, radius));
                }
            }
        }
    }
}
