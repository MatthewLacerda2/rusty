//! src/editor/inspector/components/camera/ — the Camera inspector card (this
//! `mod.rs`) plus the Visual Correction card (`visual_correction`), split apart to
//! stay under the size cap. Both cards live on the same camera entity.

use egui_phosphor::regular as icon;

mod visual_correction;

pub use visual_correction::draw_visual_correction;

use crate::editor::inspector::components::card::component_card;
use crate::editor::theme;
use crate::scene::authoring::camera as camera_ops;
use crate::scene::{ClearFlags, Entity};

/// Camera Component panel. `named_layers` are the `(index, label)` slots offered in
/// the culling-mask checklist (layer 0 + named user slots).
pub fn draw_camera(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    named_layers: &[(u8, String)],
    is_dirty: &mut bool,
) {
    let Some(cam) = entity.camera.clone() else {
        return;
    };
    // Force high-quality motion blur by default under the hood, via the shared ops
    // (so the editor and `Graphics.*` write these through one path).
    if !cam.motion_blur_active {
        camera_ops::set_motion_blur_active(entity, true);
    }
    if cam.motion_blur_samples != 64 {
        camera_ops::set_motion_blur_samples(entity, 64);
    }

    let mut remove = false;
    component_card(ui, icon::VIDEO_CAMERA, "Camera", Some(&mut remove), |ui| {
        draw_projection(ui, entity, &cam, is_dirty);
        draw_culling_mask(ui, entity, cam.culling_mask, named_layers, is_dirty);
        draw_stacking(ui, entity, cam.render_order, cam.clear_flags, is_dirty);
        ui.colored_label(
            theme::from_ui(ui).accent_blue,
            "✔ Intrinsic Motion Blur (Active | 64 Samples)",
        );
    });
    if remove {
        entity.camera = None;
        entity.visual_correction = None;
        *is_dirty = true;
    }
}

/// The projection widgets (FOV, near/far clip), reading the snapshot and routing each
/// write through the shared op.
fn draw_projection(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    cam: &crate::scene::CameraComponent,
    is_dirty: &mut bool,
) {
    let mut fov = cam.fov;
    ui.horizontal(|ui| {
        ui.label("FOV:");
        if ui.add(egui::Slider::new(&mut fov, 1.0..=120.0)).changed() {
            camera_ops::set_fov(entity, fov);
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
                    .clamp_range(0.01..=10.0),
            )
            .changed()
        {
            camera_ops::set_near(entity, near);
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
                    .clamp_range(1.0..=1000.0),
            )
            .changed()
        {
            camera_ops::set_far(entity, far);
            *is_dirty = true;
        }
    });
}

/// The Unity-style culling-mask checklist: one checkbox per offered layer, plus
/// Everything/Nothing shortcuts. Each toggle computes the next mask from `mask`
/// (the snapshot) and routes it through the shared op.
fn draw_culling_mask(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    mask: u32,
    named_layers: &[(u8, String)],
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Culling Mask");
    ui.horizontal(|ui| {
        if ui.small_button("Everything").clicked() {
            camera_ops::set_culling_mask(entity, u32::MAX);
            *is_dirty = true;
        }
        if ui.small_button("Nothing").clicked() {
            camera_ops::set_culling_mask(entity, 0);
            *is_dirty = true;
        }
    });
    for (index, label) in named_layers {
        let bit = 1u32 << *index;
        let mut on = mask & bit != 0;
        if ui.checkbox(&mut on, label).changed() {
            let next = if on { mask | bit } else { mask & !bit };
            camera_ops::set_culling_mask(entity, next);
            *is_dirty = true;
        }
    }
}

/// Camera-stack controls (#93): the stacking `render_order` (Unity "Depth") and the
/// `clear_flags` that decide how this camera composites over the ones below it.
/// Reads the snapshot values; routes writes through the shared ops.
fn draw_stacking(
    ui: &mut egui::Ui,
    entity: &mut Entity,
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
            camera_ops::set_render_order(entity, render_order);
            *is_dirty = true;
        }
    });
    let mut clear_flags = clear_flags;
    ui.horizontal(|ui| {
        ui.label("Clear Flags:");
        egui::ComboBox::from_id_source("clear_flags")
            .selected_text(match clear_flags {
                ClearFlags::Skybox => "Skybox",
                ClearFlags::SolidColor => "Solid Color",
                ClearFlags::DepthOnly => "Depth Only",
            })
            .show_ui(ui, |ui| {
                for (value, label) in [
                    (ClearFlags::Skybox, "Skybox"),
                    (ClearFlags::SolidColor, "Solid Color"),
                    (ClearFlags::DepthOnly, "Depth Only"),
                ] {
                    if ui
                        .selectable_value(&mut clear_flags, value, label)
                        .changed()
                    {
                        camera_ops::set_clear_flags(entity, clear_flags);
                        *is_dirty = true;
                    }
                }
            });
    });
}
