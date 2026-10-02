//! src/physics/follow.rs — colliders that follow animated bones (#464).
//!
//! A tick runs `Update` scripts → the physics step → the Animator → `LateUpdate`.
//! The step pushes every pose into rapier at its *start*, so the bones the
//! Animator (and `LateUpdate`) moved afterwards would otherwise be queried where
//! they stood a tick earlier — a headshot would hit last frame's head. After the
//! tick's last bone writer, [`PhysicsWorld::follow_bones`] moves every collider
//! that hangs from a bone to its bone's current pose and refits the query
//! pipeline, so the next tick's casts test the pose that was rendered.
//!
//! Only query poses move here: bodies are untouched (the next step still drives
//! them, kinematic velocities included), and a body's own collider — a
//! Rigidbody's or a CharacterController's — is the step's to place.

use std::collections::BTreeSet;

use super::compound::world_pose;
use super::convert::to_iso;
use super::world::PhysicsWorld;
use crate::scene::Scene;

impl PhysicsWorld {
    /// Re-place each collider on or under a bone at its live world pose and refit
    /// the query pipeline over the ones that moved. Walks the sorted plan, so the
    /// order is deterministic; a scene without skeletons costs one empty set.
    pub fn follow_bones(&mut self, scene: &Scene) {
        let bones = scene.bone_ids();
        if bones.is_empty() {
            return;
        }
        let mut moved = Vec::new();
        for (&owner, ids) in &self.plan {
            for &id in ids {
                let carries_body = id == owner
                    && (scene.world.has_rigidbody(id) || scene.world.has_character_controller(id));
                if carries_body || !hangs_from_bone(scene, &bones, id) {
                    continue;
                }
                let (Some(&handle), Some(pose)) =
                    (self.id_to_collider.get(&id), world_pose(scene, id))
                else {
                    continue;
                };
                let Some(collider) = self.colliders.get_mut(handle) else {
                    continue;
                };
                let target = to_iso(pose.pos, pose.rot);
                if *collider.position() != target {
                    collider.set_position(target);
                    moved.push(handle);
                }
            }
        }
        if !moved.is_empty() {
            self.query_pipeline
                .update_incremental(&self.colliders, &moved, &[], true);
        }
    }
}

/// Whether `id` is a bone or sits anywhere under one.
fn hangs_from_bone(scene: &Scene, bones: &BTreeSet<u32>, id: u32) -> bool {
    let mut current = Some(id);
    while let Some(c) = current {
        if bones.contains(&c) {
            return true;
        }
        current = scene.world.parent_id(c);
    }
    false
}
