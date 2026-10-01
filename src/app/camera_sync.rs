//! src/app/camera_sync.rs — reconcile the render camera with the scene's camera.
//!
//! The render [`Camera`] resource carries the viewport's position/orientation
//! (driven by the editor free-fly cam or the play-mode follow logic) plus its lens
//! and culling mask. Each frame this copies the active `CameraComponent`'s
//! fov/near/far/culling-mask into that resource, so per-camera clip planes and the
//! Unity-style culling mask (#92) take effect without the renderer reaching into the
//! scene. Multi-camera stacking is the follow-up (#93).
//!
//! [`Camera`]: crate::scene::Camera

use glam::Vec3;

use crate::scene::sync_lens_from_scene;

use super::GameWorld;

impl GameWorld {
    /// Sync the render camera's lens + culling mask from the scene's active camera.
    pub(crate) fn sync_render_camera(&mut self) {
        let scene = self.world.scene.borrow();
        let mut camera = self.resources.camera.borrow_mut();
        sync_lens_from_scene(&mut camera, &scene, self.resources.is_playing);
    }

    /// Hand the sim's game time to the renderer (#398) as `Scene::shader_time`, which
    /// it uploads as `camera.time`. Zero outside Play, so edit-mode and preview shots
    /// stay pixel-comparable; the renderer only reads it, never owns the clock.
    pub(crate) fn sync_shader_time(&mut self) {
        let time = if self.resources.is_playing {
            self.resources.time.borrow().time as f32
        } else {
            0.0
        };
        self.world.scene.borrow_mut().shader_time = time;
    }

    /// Editor-mode free-fly camera (WASD + arrow look). Runs each frame while not in
    /// Play; no entity simulation. Lives here with the other camera reconciliation.
    pub(crate) fn editor_fly(&mut self, dt: f32) {
        let inp = self.resources.input.borrow();
        let mut cam = self.resources.camera.borrow_mut();
        let mut move_dir = Vec3::ZERO;
        if inp.is_key_down("W") {
            move_dir += cam.forward();
        }
        if inp.is_key_down("S") {
            move_dir -= cam.forward();
        }
        if inp.is_key_down("A") {
            move_dir -= cam.right();
        }
        if inp.is_key_down("D") {
            move_dir += cam.right();
        }
        if move_dir.length_squared() > 0.001 {
            cam.position += move_dir.normalize() * 10.0 * dt;
        }

        let look = 90.0 * dt;
        if inp.is_key_down("LEFT") {
            cam.yaw -= look;
        }
        if inp.is_key_down("RIGHT") {
            cam.yaw += look;
        }
        if inp.is_key_down("UP") {
            cam.pitch += look;
        }
        if inp.is_key_down("DOWN") {
            cam.pitch -= look;
        }
        cam.pitch = cam.pitch.clamp(-80.0, 80.0);
    }
}

#[cfg(test)]
#[path = "shader_time_tests.rs"]
mod shader_time_tests;
