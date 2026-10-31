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

use super::compound::world_pose;
use super::convert::to_pose;
use super::world::PhysicsWorld;
use crate::scene::Scene;

impl PhysicsWorld {
    /// Re-place each collider on or under a bone at its live world pose and refit
    /// the query pipeline over the ones that moved. Walks the skeletons' subtrees
    /// ([`Scene::bone_subtrees`], ascending ids), never the scene's other
    /// colliders, so a map's static colliders cost nothing here (#1617).
    pub fn follow_bones(&mut self, scene: &Scene) {
        let mut moved = Vec::new();
        for id in scene.bone_subtrees() {
            let Some(&handle) = self.id_to_collider.get(&id) else {
                continue;
            };
            let carries_body = self.plan.contains_key(&id)
                && (scene.world.has_rigidbody(id) || scene.world.has_character_controller(id));
            if carries_body {
                continue;
            }
            let (Some(pose), Some(collider)) =
                (world_pose(scene, id), self.colliders.get_mut(handle))
            else {
                continue;
            };
            let target = to_pose(pose.pos, pose.rot);
            if *collider.position() != target {
                collider.set_position(target);
                moved.push(handle);
            }
        }
        if !moved.is_empty() {
            self.refresh_query_tree(&moved);
        }
    }
}
