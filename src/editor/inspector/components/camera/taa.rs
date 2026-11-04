//! The Camera card's TAA toggle (#1702): temporal anti-aliasing, which replaces
//! FXAA while it runs. Writes through the shared op, so this and
//! `Graphics.SetTaaActive` are one path.

use crate::scene::authoring::rendering::camera as camera_ops;
use crate::scene::CameraComponent;

pub(super) fn draw_taa(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    cam: &CameraComponent,
    is_dirty: &mut bool,
) {
    let mut on = cam.taa_active;
    let toggle = ui.checkbox(&mut on, "TAA (Temporal Anti-Aliasing)");
    let toggle = toggle.on_hover_text("Replaces FXAA while it runs; the Scene view never runs it");
    if toggle.changed() {
        if let Some(mut c) = world.camera_mut(id) {
            camera_ops::set_taa_active(&mut c, on);
        }
        *is_dirty = true;
    }
}
