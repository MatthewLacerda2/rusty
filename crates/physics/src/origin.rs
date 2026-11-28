//! A floating-origin shift of the rapier world in one pass: every body moves by the shift,
//! velocities and sleep kept, and the change entries the scene's shift stamped are read
//! past, so the next sync re-reads no static. On the full-size city that is what keeps
//! a shift's next step from re-reading every collider's state.

use glam::Vec3;

use super::world::PhysicsWorld;
use rusty_scene::Scene;

impl PhysicsWorld {
    /// Before the scene's shift: what the tick changed reaches rapier first, so nothing but
    /// the shift is left in the entries `shift_origin` reads past.
    pub fn prepare_shift(&mut self, scene: &mut Scene) {
        self.sync_to_rapier(scene);
    }

    pub fn shift_origin(&mut self, scene: &mut Scene, by: Vec3) {
        for (_, body) in self.bodies.iter_mut() {
            let mut pose = *body.position();
            pose.translation += by;
            body.set_position(pose, false);
        }
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
