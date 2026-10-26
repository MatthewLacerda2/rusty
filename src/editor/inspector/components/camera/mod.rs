//! src/editor/inspector/components/camera/ — the Camera inspector card (this
//! `mod.rs`) plus the Post Processing card (`post_processing`, with its colour
//! grade in `grading` and its custom effects list in `custom_effects`), split apart to stay under the size cap. Both
//! cards live on the same camera entity.

use crate::editor::widgets::DragRange;
use egui_phosphor::regular as icon;

mod custom_effects;
mod grading;
mod post_processing;
mod target;

pub use post_processing::draw_post_processing;

use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::{self, rendering::camera as camera_ops, ComponentKind};
use crate::scene::ClearFlags;

/// Camera Component panel. `named_layers` are the `(index, label)` slots offered in
/// the culling-mask checklist (layer 0 + named user slots).
pub fn draw_camera(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    named_layers: &[(u8, String)],
    is_dirty: &mut bool,
) {
    let Some(cam) = world.camera(id).map(|c| c.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(ui, icon::VIDEO_CAMERA, "Camera", Some(&mut remove), |ui| {
        target::draw_projection_mode(ui, world, id, &cam, is_dirty);
        draw_projection(ui, world, id, &cam, is_dirty);
        draw_culling_mask(ui, world, id, cam.culling_mask, named_layers, is_dirty);
        draw_stacking(ui, world, id, cam.render_order, cam.clear_flags, is_dirty);
        draw_motion_blur(ui, world, id, &cam, is_dirty);
        draw_fxaa(ui, world, id, cam.fxaa_active, is_dirty);
        target::draw_target(ui, world, id, &cam, is_dirty);
    });
    if remove {
        // Cascade to camera-dependent components (PostProcessing) from the
        // declaration (#359), not a hand-written pair of clears.
        authoring::remove_with_cascade(world, id, ComponentKind::Camera);
        *is_dirty = true;
    }
}

/// The motion-blur toggle and its sample-count slider (#1433). Writes go through the
/// shared ops, so this card and `Graphics.SetMotionBlur*` are one path; the op owns
/// the sample clamp.
fn draw_motion_blur(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    cam: &crate::scene::CameraComponent,
    is_dirty: &mut bool,
) {
    let mut on = cam.motion_blur_active;
    if ui.checkbox(&mut on, "Motion Blur").changed() {
        if let Some(mut c) = world.camera_mut(id) {
            camera_ops::set_motion_blur_active(&mut c, on);
        }
        *is_dirty = true;
    }
    let mut samples = cam.motion_blur_samples;
    ui.horizontal(|ui| {
        ui.label("Samples:");
        let range = crate::scene::CameraComponent::MIN_MOTION_BLUR_SAMPLES
            ..=crate::scene::CameraComponent::MAX_MOTION_BLUR_SAMPLES;
        if ui.add(egui::Slider::new(&mut samples, range)).changed() {
            if let Some(mut c) = world.camera_mut(id) {
                camera_ops::set_motion_blur_samples(&mut c, samples);
            }
            *is_dirty = true;
        }
    });
}

/// The FXAA toggle (#360): a real checkbox, because FXAA is an effect a shot might
/// genuinely want off (a pixel-art look, or a screenshot being diffed for raw
/// edges). Writes through the shared op, so this and `Graphics.SetFxaaActive` are
/// one path.
fn draw_fxaa(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    active: bool,
    is_dirty: &mut bool,
) {
    let mut on = active;
    if ui.checkbox(&mut on, "FXAA (Anti-Aliasing)").changed() {
        if let Some(mut c) = world.camera_mut(id) {
            camera_ops::set_fxaa_active(&mut c, on);
        }
        *is_dirty = true;
    }
}

/// The projection widgets (FOV, near/far clip), reading the snapshot and routing each
/// write through the shared op.
fn draw_projection(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    cam: &crate::scene::CameraComponent,
    is_dirty: &mut bool,
) {
    let mut fov = cam.fov;
    ui.horizontal(|ui| {
        ui.label("FOV:");
        if ui
            .add(egui::Slider::new(
                &mut fov,
                camera_ops::MIN_FOV..=camera_ops::MAX_FOV,
            ))
            .changed()
        {
            if let Some(mut c) = world.camera_mut(id) {
                camera_ops::set_fov(&mut c, fov);
            }
            *is_dirty = true;
        }
    });
    let mut near = cam.near;
    ui.horizontal(|ui| {
        ui.label("Near Clip:");
        if ui
            .add(
                egui::DragValue::new(&mut near)
                    .speed(0.01)
                    .ranged(0.01..=10.0),
            )
            .changed()
        {
            if let Some(mut c) = world.camera_mut(id) {
                camera_ops::set_near(&mut c, near);
            }
            *is_dirty = true;
        }
    });
    let mut far = cam.far;
    ui.horizontal(|ui| {
        ui.label("Far Clip:");
        if ui
            .add(
                egui::DragValue::new(&mut far)
                    .speed(1.0)
                    .ranged(1.0..=1000.0),
            )
            .changed()
        {
            if let Some(mut c) = world.camera_mut(id) {
                camera_ops::set_far(&mut c, far);
            }
            *is_dirty = true;
        }
    });
}

/// The Unity-style culling-mask checklist: one checkbox per offered layer, plus
/// Everything/Nothing shortcuts. Each toggle computes the next mask from `mask`
/// (the snapshot) and routes it through the shared op.
fn draw_culling_mask(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    mask: u32,
    named_layers: &[(u8, String)],
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Culling Mask");
    ui.horizontal(|ui| {
        if ui.small_button("Everything").clicked() {
            if let Some(mut c) = world.camera_mut(id) {
                camera_ops::set_culling_mask(&mut c, u32::MAX);
            }
            *is_dirty = true;
        }
        if ui.small_button("Nothing").clicked() {
            if let Some(mut c) = world.camera_mut(id) {
                camera_ops::set_culling_mask(&mut c, 0);
            }
            *is_dirty = true;
        }
    });
    for (index, label) in named_layers {
        let bit = 1u32 << *index;
        let mut on = mask & bit != 0;
        if ui.checkbox(&mut on, label).changed() {
            let next = if on { mask | bit } else { mask & !bit };
            if let Some(mut c) = world.camera_mut(id) {
                camera_ops::set_culling_mask(&mut c, next);
            }
            *is_dirty = true;
        }
    }
}

/// Camera-stack controls (#93): the stacking `render_order` (Unity "Depth") and the
/// `clear_flags` that decide how this camera composites over the ones below it.
/// Reads the snapshot values; routes writes through the shared ops.
fn draw_stacking(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    render_order: i32,
    clear_flags: ClearFlags,
    is_dirty: &mut bool,
) {
    let mut render_order = render_order;
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        ui.label("Render Order (Depth):");
        if ui
            .add(egui::DragValue::new(&mut render_order).speed(1))
            .changed()
        {
            if let Some(mut c) = world.camera_mut(id) {
                camera_ops::set_render_order(&mut c, render_order);
            }
            *is_dirty = true;
        }
    });
    let mut clear_flags = clear_flags;
    ui.horizontal(|ui| {
        ui.label("Clear Flags:");
        egui::ComboBox::from_id_salt("clear_flags")
            .selected_text(clear_flags.name())
            .show_ui(ui, |ui| {
                for value in ClearFlags::ALL {
                    if ui
                        .selectable_value(&mut clear_flags, value, value.name())
                        .changed()
                    {
                        if let Some(mut c) = world.camera_mut(id) {
                            camera_ops::set_clear_flags(&mut c, clear_flags);
                        }
                        *is_dirty = true;
                    }
                }
            });
    });
}
