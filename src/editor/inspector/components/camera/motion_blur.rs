//! The Camera card's motion-blur widgets (#1433, #1436): the toggle, the quality
//! level (#1522) with the `Custom` sample count, the intensity (shutter fraction)
//! and the clamp (longest streak). Writes go
//! through the shared ops, so this card and `Graphics.SetMotionBlur*` are one path;
//! each op owns its clamp.

use super::post_processing::quality_row;
use crate::core::settings::quality::EffectQuality;
use crate::scene::authoring::rendering::camera as camera_ops;
use crate::scene::CameraComponent;

/// The quality dropdown (#1522).
fn draw_quality(
    ui: &mut egui::Ui,
    (world, id): (&mut crate::ecs::World, u32),
    cam: &CameraComponent,
    is_dirty: &mut bool,
) {
    if let Some(quality) = quality_row(ui, "Motion Blur Quality", cam.motion_blur_quality) {
        if let Some(mut c) = world.camera_mut(id) {
            camera_ops::set_motion_blur_quality(&mut c, quality);
        }
        *is_dirty = true;
    }
}

/// The motion-blur toggle, its quality and its sliders.
pub(super) fn draw_motion_blur(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    cam: &CameraComponent,
    is_dirty: &mut bool,
) {
    let mut on = cam.motion_blur_active;
    if ui.checkbox(&mut on, "Motion Blur").changed() {
        if let Some(mut c) = world.camera_mut(id) {
            camera_ops::set_motion_blur_active(&mut c, on);
        }
        *is_dirty = true;
    }
    draw_quality(ui, (&mut *world, id), cam, is_dirty);
    let mut samples = cam.motion_blur_samples;
    let range = CameraComponent::MIN_MOTION_BLUR_SAMPLES..=CameraComponent::MAX_MOTION_BLUR_SAMPLES;
    if cam.motion_blur_quality == EffectQuality::Custom
        && slider(
            ui,
            "Samples:",
            crate::editor::widgets::slider(&mut samples, range),
        )
    {
        if let Some(mut c) = world.camera_mut(id) {
            camera_ops::set_motion_blur_samples(&mut c, samples);
        }
        *is_dirty = true;
    }
    let mut intensity = cam.motion_blur_intensity;
    let range = 0.0..=CameraComponent::MAX_MOTION_BLUR_INTENSITY;
    if slider(
        ui,
        "Intensity:",
        crate::editor::widgets::slider(&mut intensity, range),
    ) {
        if let Some(mut c) = world.camera_mut(id) {
            camera_ops::set_motion_blur_intensity(&mut c, intensity);
        }
        *is_dirty = true;
    }
    let mut max_length = cam.motion_blur_max_length;
    let range = 0.0..=CameraComponent::MAX_MOTION_BLUR_MAX_LENGTH;
    if slider(
        ui,
        "Clamp:",
        crate::editor::widgets::slider(&mut max_length, range),
    ) {
        if let Some(mut c) = world.camera_mut(id) {
            camera_ops::set_motion_blur_max_length(&mut c, max_length);
        }
        *is_dirty = true;
    }
}

/// One labelled slider row; whether it changed.
fn slider(ui: &mut egui::Ui, label: &str, slider: egui::Slider) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(slider).changed()
    })
    .inner
}
