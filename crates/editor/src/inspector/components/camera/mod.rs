//! crates/editor/src/inspector/components/camera/ — the Camera inspector card (this
//! `mod.rs`) plus the Post Processing card (`post_processing`, one section per post
//! effect). Both cards live on the same camera entity. The field of view and its
//! axis are drawn by `fov`. The Camera card draws the
//! camera effects' sections (`CAMERA_SECTIONS`: motion blur in `motion_blur`, TAA
//! in `taa`, FXAA here).

use crate::widgets::DragRange;
use egui_phosphor::regular as icon;

mod fov;
mod motion_blur;
mod occlusion;
pub(crate) mod post_processing;
mod taa;
mod target;

pub use post_processing::{commit_abandoned_paths, draw_post_processing};

use crate::inspector::components::card::component_card;
use rusty_scene::authoring::{self, rendering::camera as camera_ops, ComponentKind};
use rusty_scene::{CameraComponent, ClearFlags};

/// Camera Component panel. `named_layers` are the `(index, label)` slots offered in
/// the culling-mask checklist (layer 0 + named user slots); `view_aspect` is the
/// editor viewport's width / height, which the field of view converts at (#2038).
pub fn draw_camera(
    ui: &mut egui::Ui,
    world: &mut rusty_ecs::World,
    id: u32,
    named_layers: &[(u8, String)],
    view_aspect: f32,
    is_dirty: &mut bool,
) {
    let Some(cam) = world.camera(id).map(|c| c.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(ui, icon::VIDEO_CAMERA, "Camera", Some(&mut remove), |ui| {
        target::draw_projection_mode(ui, world, id, &cam, is_dirty);
        draw_projection(ui, world, id, &cam, view_aspect, is_dirty);
        draw_culling_mask(ui, world, id, cam.culling_mask, named_layers, is_dirty);
        occlusion::draw_occlusion_culling(ui, world, id, &cam, is_dirty);
        draw_stacking(ui, world, id, cam.render_order, cam.clear_flags, is_dirty);
        for section in camera_effects() {
            ui.push_id(section.effect, |ui| {
                (section.draw)(ui, world, id, &cam, is_dirty)
            });
        }
        target::draw_target(ui, world, id, &cam, is_dirty);
    });
    if remove {
        // Cascade to camera-dependent components (PostProcessing) from the
        // declaration (#359), not a hand-written pair of clears.
        authoring::remove_with_cascade(world, id, ComponentKind::Camera);
        *is_dirty = true;
    }
}

/// One camera effect's section of the Camera card (#1467): motion blur, FXAA and TAA,
/// whose settings live on the camera, not on a volume.
pub(crate) struct CameraSection {
    /// The effect, as `render::postfx::effects` names it.
    pub effect: &'static str,
    /// Where it sits among the camera effects: lower first.
    pub order: u16,
    pub draw: fn(&mut egui::Ui, &mut rusty_ecs::World, u32, &CameraComponent, &mut bool),
}

/// Every post effect the Inspector draws a section for, on either card (the volume's
/// and the camera's): what the post effects' completeness gate checks the renderer
/// against (`render::postfx::completeness`).
pub fn drawn_effects() -> Vec<&'static str> {
    let volume = post_processing::SECTIONS.iter().map(|s| s.effect);
    volume
        .chain(CAMERA_SECTIONS.iter().map(|s| s.effect))
        .collect()
}

/// The camera effects' sections, by effect.
pub(crate) const CAMERA_SECTIONS: &[CameraSection] = &[
    // sorted: one row per camera effect; the card orders them by `order`
    CameraSection {
        effect: "fxaa",
        order: 2,
        draw: draw_fxaa,
    },
    CameraSection {
        effect: "motion_blur",
        order: 1,
        draw: motion_blur::draw_motion_blur,
    },
    CameraSection {
        effect: "taa",
        order: 3,
        draw: taa::draw_taa,
    },
];

/// [`CAMERA_SECTIONS`] in the card's order.
fn camera_effects() -> Vec<&'static CameraSection> {
    let mut sections: Vec<&CameraSection> = CAMERA_SECTIONS.iter().collect();
    sections.sort_by_key(|s| s.order);
    sections
}

/// The FXAA toggle (#360): a real checkbox, because FXAA is an effect a shot might
/// genuinely want off (a pixel-art look, or a screenshot being diffed for raw
/// edges). Writes through the shared op, so this and `Graphics.SetFxaaActive` are
/// one path.
fn draw_fxaa(
    ui: &mut egui::Ui,
    world: &mut rusty_ecs::World,
    id: u32,
    cam: &CameraComponent,
    is_dirty: &mut bool,
) {
    let mut on = cam.fxaa_active;
    if ui.checkbox(&mut on, "FXAA (Anti-Aliasing)").changed() {
        if let Some(mut c) = world.camera_mut(id) {
            camera_ops::set_fxaa_active(&mut c, on);
        }
        *is_dirty = true;
    }
}

/// The projection widgets (FOV and its axis, near/far clip), reading the snapshot
/// and routing each write through the shared op. `view_aspect` is the viewport's.
fn draw_projection(
    ui: &mut egui::Ui,
    world: &mut rusty_ecs::World,
    id: u32,
    cam: &rusty_scene::CameraComponent,
    view_aspect: f32,
    is_dirty: &mut bool,
) {
    fov::draw_fov(ui, world, id, cam, fov::aspect(cam, view_aspect), is_dirty);
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
    world: &mut rusty_ecs::World,
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
    world: &mut rusty_ecs::World,
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

impl crate::inspector::components::add::AddEntry for rusty_components::CameraComponent {
    const CATEGORY: crate::inspector::components::add::Category =
        crate::inspector::components::add::Category::Rendering;
    const LABEL: &'static str = "Camera Component";
}
