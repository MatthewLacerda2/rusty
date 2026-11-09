//! The Camera card's Occlusion Culling toggle (#1714, Unity's camera has the same
//! box): whether the camera skips what the scene's baked PVS hides from its cell,
//! and what its GPU depth pyramid hides (#1963).
//! Writes through the shared op, so this and `Camera.SetUseOcclusionCulling` are
//! one path.

use crate::scene::authoring::rendering::camera as camera_ops;
use crate::scene::CameraComponent;

pub(super) fn draw_occlusion_culling(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    cam: &CameraComponent,
    is_dirty: &mut bool,
) {
    let mut on = cam.use_occlusion_culling;
    let toggle = ui.checkbox(&mut on, "Occlusion Culling");
    let toggle = toggle.on_hover_text(
        "Skip meshes hidden from this camera: by the baked occlusion (Generate Lighting), \
         and by the GPU's depth test (the quality preset's GPU occlusion culling)",
    );
    if toggle.changed() {
        if let Some(mut c) = world.camera_mut(id) {
            camera_ops::set_use_occlusion_culling(&mut c, on);
        }
        *is_dirty = true;
    }
}
