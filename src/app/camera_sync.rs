//! src/app/camera_sync.rs — reconcile the render camera with the scene's camera.
//!
//! The render [`Camera`] resource carries the viewport's position/orientation
//! (driven by the editor free-fly cam or the play-mode follow logic) plus its lens
//! and culling mask. Each frame this copies the active `CameraComponent`'s
//! fov/near/far/culling-mask into that resource, so per-camera clip planes and the
//! Unity-style culling mask (#92) take effect without the renderer reaching into the
//! scene. Multi-camera stacking is the follow-up (#93).
//!
//! [`Camera`]: crate::render::Camera

use crate::render::sync_lens_from_scene;

use super::GameWorld;

impl GameWorld {
    /// Sync the render camera's lens + culling mask from the scene's active camera.
    pub(crate) fn sync_render_camera(&mut self) {
        let scene = self.world.scene.borrow();
        let mut camera = self.resources.camera.borrow_mut();
        sync_lens_from_scene(&mut camera, &scene, self.resources.is_playing);
    }
}
