//! src/app/camera_sync.rs — reconcile the render camera with the scene's camera.
//!
//! The render [`Camera`] resource carries the game camera's position/orientation
//! (driven by the play-mode follow logic and `Camera.*`) plus its lens
//! and culling mask. Each frame this copies the active `CameraComponent`'s
//! fov/near/far/culling-mask into that resource, so per-camera clip planes and the
//! Unity-style culling mask (#92) take effect without the renderer reaching into the
//! scene. Multi-camera stacking is the follow-up (#93).
//!
//! [`Camera`]: crate::scene::Camera

use crate::scene::sync_lens_from_scene;

use super::GameWorld;

impl GameWorld {
    /// Sync the render camera's lens + culling mask from the scene's active camera.
    pub(crate) fn sync_render_camera(&mut self) {
        let scene = self.world.scene.borrow();
        let mut camera = self.resources.camera.borrow_mut();
        sync_lens_from_scene(&mut camera, &scene, self.resources.is_playing);
    }

    /// Hand the sim's clocks to the renderer: game time (#398) as
    /// `Scene::shader_time`, which it uploads as `camera.time`, and unscaled time as
    /// `Scene::ui_time` for custom UI shaders (#427). Zero outside Play, so edit-mode
    /// and preview shots stay pixel-comparable; the renderer only reads them, never
    /// owns the clock.
    pub(crate) fn sync_shader_time(&mut self) {
        let (time, ui_time) = if self.resources.is_playing {
            let t = self.resources.time.borrow();
            (t.time as f32, t.unscaled_time as f32)
        } else {
            (0.0, 0.0)
        };
        let mut scene = self.world.scene.borrow_mut();
        scene.shader_time = time;
        scene.ui_time = ui_time;
    }
}

#[cfg(test)]
#[path = "shader_time_tests.rs"]
mod shader_time_tests;
