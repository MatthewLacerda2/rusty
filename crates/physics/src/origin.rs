//! A floating-origin shift of the rapier world in one pass: every body moves by the shift,
//! velocities and sleep kept, and the change entries the scene's shift stamped are read
//! past, so the next sync re-reads no static (on the full-size city, every collider).

use glam::Vec3;

use super::world::{body_to_write, PhysicsWorld};
use super::writes::collider_to_write;
use rusty_scene::Scene;

impl PhysicsWorld {
    /// Before the scene's shift: what the tick changed reaches rapier first, so nothing but
    /// the shift is left in the entries `shift_origin` reads past.
    pub fn prepare_shift(&mut self, scene: &mut Scene) {
        self.sync_to_rapier(scene);
    }

    pub fn shift_origin(&mut self, scene: &mut Scene, by: Vec3) {
        let handles: Vec<_> = self.bodies.iter().map(|(h, _)| h).collect();
        let mut moved = Vec::new();
        for handle in handles {
            if let Some(body) = body_to_write(&mut self.bodies, handle, |_| by != Vec3::ZERO) {
                let mut pose = *body.position();
                pose.translation += by;
                body.set_position(pose, false);
                moved.extend(body.colliders().iter().map(|&c| (c, pose)));
            }
        }
        // Rapier places colliders only in `step`; the next tick queries before it.
        for &(handle, pose) in &moved {
            let Some(collider) = collider_to_write(&mut self.colliders, handle, |_| true) else {
                continue;
            };
            if let Some(&offset) = collider.position_wrt_parent() {
                collider.set_position(pose * offset);
            }
        }
        let colliders: Vec<_> = moved.into_iter().map(|(c, _)| c).collect();
        self.refresh_query_tree(&colliders);
        for pose in self.tracked.static_poses.values_mut() {
            pose.translation += by;
        }
        let world = &mut scene.world;
        if let Some(watch) = self.tracked.watch.as_mut() {
            if watch.read(world).is_none() {
                // A log it could not read: the next sync resyncs in full, as it would have.
                self.tracked.watch = None;
            }
        }
        if let Some(cuts) = self.tracked.cuts.as_mut() {
            let _ = world.changes(cuts);
        }
    }
}

#[cfg(test)]
#[path = "origin_tests.rs"]
mod origin_tests;
