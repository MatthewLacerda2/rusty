use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use crate::compound::{relative_pose, world_pose};
use crate::convert::to_pose;
use crate::shape::build::ColliderInputs;
use crate::watch::Owners;
use crate::world::{collider_to_write, PhysicsWorld};
use rusty_components::{capsule_dims, CapsuleAxis, ColliderShape, PhysicsMaterial};
use rusty_ecs::World;
use rusty_scene::Scene;

pub(crate) fn character_collider_inputs(world: &World, id: u32) -> Option<ColliderInputs> {
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

/// Unrotated in the world: its rotation on the body undoes the owner's.
pub(crate) fn upright_offset(
    owner_rot: Quat,
    offset_pos: Vec3,
    offset_rot: Quat,
    center: Vec3,
) -> Pose {
    to_pose(offset_pos + offset_rot * center, owner_rot.inverse())
}

impl PhysicsWorld {
    pub(crate) fn sync_characters(&mut self, scene: &Scene, owners: &Owners) {
        for (&owner, ids) in owners.pick(&self.plan) {
            for &id in ids {
                let Some(cc) = scene.world.character_controller(id).map(|c| c.clone()) else {
                    continue;
                };
                let (Some(owner_pose), Some(pose)) =
                    (world_pose(scene, owner), world_pose(scene, id))
                else {
                    continue;
                };
                let Some(&handle) = self.id_to_collider.get(&id) else {
                    continue;
                };
                let (pos, rot) = relative_pose(&owner_pose, &pose);
                let offset = upright_offset(owner_pose.rot, pos, rot, cc.center * pose.scale);
                let (half_segment, radius) =
                    capsule_dims(cc.radius, cc.height, CapsuleAxis::Y, pose.scale);
                let sized = |c: &Collider| {
                    c.shape().as_capsule().is_some_and(|c| {
                        (c.half_height() - half_segment).abs() < 1e-6
                            && (c.radius - radius).abs() < 1e-6
                    })
                };
                let placed = |c: &Collider| c.position_wrt_parent() == Some(&offset);
                let differs = |c: &Collider| !placed(c) || !sized(c);
                let Some(collider) = collider_to_write(&mut self.colliders, handle, differs) else {
                    continue;
                };
                if !placed(collider) {
                    collider.set_position_wrt_parent(offset);
                }
                if !sized(collider) {
                    collider.set_shape(SharedShape::capsule_y(half_segment, radius));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The capsule rides at the child's offset plus its centre, upright in the world.
    #[test]
    fn the_capsule_rides_its_offset_and_stays_upright() {
        let owner = Quat::from_rotation_y(0.9);
        let child = Quat::from_rotation_z(0.4);
        let pos = Vec3::new(1.0, 2.0, -0.5);
        let center = Vec3::new(0.0, 1.0, 0.25);
        let pose = upright_offset(owner, pos, child, center);
        let want = pos + child * center;
        assert!((pose.translation - want).length() < 1e-5, "{pose:?}");
        assert!((owner * pose.rotation).angle_between(Quat::IDENTITY) < 1e-5);
    }
}
