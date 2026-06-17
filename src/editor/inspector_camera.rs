use egui_phosphor::regular as icon;

use crate::editor::inspector_card::component_card;
use crate::editor::theme;
use crate::scene::{ClearFlags, Entity, Tonemap};

/// Camera Component panel. `named_layers` are the `(index, label)` slots offered in
/// the culling-mask checklist (layer 0 + named user slots).
pub fn draw_camera(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    named_layers: &[(u8, String)],
    is_dirty: &mut bool,
) {
    if entity.camera.is_none() {
        return;
    }
    let mut remove = false;
    component_card(ui, icon::VIDEO_CAMERA, "Camera", Some(&mut remove), |ui| {
        let Some(cam) = &mut entity.camera else {
            return;
        };
        // Force high-quality motion blur by default under the hood
        cam.motion_blur_active = true;
        cam.motion_blur_samples = 64;

        ui.horizontal(|ui| {
            ui.label("FOV:");
            ui.add(egui::Slider::new(&mut cam.fov, 1.0..=120.0));
        });
        ui.horizontal(|ui| {
            ui.label("Near Clip:");
            ui.add(
                egui::DragValue::new(&mut cam.near)
                    .speed(0.01)
                    .clamp_range(0.01..=10.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Far Clip:");
            ui.add(
                egui::DragValue::new(&mut cam.far)
                    .speed(1.0)
                    .clamp_range(1.0..=1000.0),
            );
        });
        draw_culling_mask(ui, &mut cam.culling_mask, named_layers, is_dirty);
        draw_stacking(ui, &mut cam.render_order, &mut cam.clear_flags, is_dirty);
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

/// The Unity-style culling-mask checklist: one checkbox per offered layer, plus
/// Everything/Nothing shortcuts. Toggling a layer flips its bit in `mask`.
fn draw_culling_mask(
    ui: &mut egui::Ui,
    mask: &mut u32,
    named_layers: &[(u8, String)],
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Culling Mask");
    ui.horizontal(|ui| {
        if ui.small_button("Everything").clicked() {
            *mask = u32::MAX;
            *is_dirty = true;
        }
        if ui.small_button("Nothing").clicked() {
            *mask = 0;
            *is_dirty = true;
        }
    });
    for (index, label) in named_layers {
        let bit = 1u32 << *index;
        let mut on = *mask & bit != 0;
        if ui.checkbox(&mut on, label).changed() {
            if on {
                *mask |= bit;
            } else {
                *mask &= !bit;
            }
            *is_dirty = true;
        }
    }
}

/// Camera-stack controls (#93): the stacking `render_order` (Unity "Depth") and the
/// `clear_flags` that decide how this camera composites over the ones below it.
fn draw_stacking(
    ui: &mut egui::Ui,
    render_order: &mut i32,
    clear_flags: &mut ClearFlags,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        ui.label("Render Order (Depth):");
        if ui
            .add(egui::DragValue::new(render_order).speed(1))
            .changed()
        {
            *is_dirty = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label("Clear Flags:");
        egui::ComboBox::from_id_source("clear_flags")
            .selected_text(match clear_flags {
                ClearFlags::Skybox => "Skybox",
                ClearFlags::SolidColor => "Solid Color",
                ClearFlags::DepthOnly => "Depth Only",
            })
            .show_ui(ui, |ui| {
                let mut pick = |ui: &mut egui::Ui, value, label| {
                    if ui.selectable_value(clear_flags, value, label).changed() {
                        *is_dirty = true;
                    }
                };
                pick(ui, ClearFlags::Skybox, "Skybox");
                pick(ui, ClearFlags::SolidColor, "Solid Color");
                pick(ui, ClearFlags::DepthOnly, "Depth Only");
            });
    });
}

/// Visual Correction Component panel
#[allow(clippy::too_many_lines)] // legacy; burn down in #124
pub fn draw_visual_correction(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    if entity.visual_correction.is_none() {
        return;
    }
    let mut remove = false;
    component_card(
        ui,
        icon::SPARKLE,
        "Visual Correction",
        Some(&mut remove),
        |ui| {
            let Some(vc) = &mut entity.visual_correction else {
                return;
            };
            ui.checkbox(&mut vc.active, "Enable Visual Correction");

            ui.add_space(3.0);
            ui.label("Bloom");
            ui.checkbox(&mut vc.bloom_active, "  Bloom Active");
            if vc.bloom_active {
                ui.horizontal(|ui| {
                    ui.label("    Intensity:");
                    ui.add(egui::Slider::new(&mut vc.bloom_intensity, 0.0..=5.0));
                });
                ui.horizontal(|ui| {
                    ui.label("    Threshold:");
                    ui.add(egui::Slider::new(&mut vc.bloom_threshold, 0.0..=1.0));
                });
            }

            ui.add_space(3.0);
            ui.label("Color Correction");
            ui.horizontal(|ui| {
                ui.label("  Exposure (EV):");
                ui.add(egui::Slider::new(&mut vc.exposure, -4.0..=4.0));
            });
            ui.horizontal(|ui| {
                ui.label("  Contrast:");
                ui.add(egui::Slider::new(&mut vc.contrast, 0.0..=2.0));
            });
            ui.horizontal(|ui| {
                ui.label("  Saturation:");
                ui.add(egui::Slider::new(&mut vc.saturation, 0.0..=2.0));
            });
            ui.horizontal(|ui| {
                ui.label("  Tonemap:");
                egui::ComboBox::from_id_source("tonemap")
                    .selected_text(format!("{:?}", vc.tonemap))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut vc.tonemap, Tonemap::Aces, "ACES");
                        ui.selectable_value(&mut vc.tonemap, Tonemap::Reinhard, "Reinhard");
                        ui.selectable_value(&mut vc.tonemap, Tonemap::None, "None");
                    });
            });
            ui.horizontal(|ui| {
                ui.label("  Gamma:");
                ui.add(egui::Slider::new(&mut vc.gamma, 1.0..=3.0));
            });

            ui.add_space(3.0);
            ui.label("Screen Space Reflections (SSR)");
            ui.checkbox(&mut vc.ssr_active, "  SSR Active");
            if vc.ssr_active {
                ui.horizontal(|ui| {
                    ui.label("    Quality:");
                    egui::ComboBox::from_label("")
                        .selected_text(&vc.ssr_quality)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut vc.ssr_quality, "Low".to_string(), "Low");
                            ui.selectable_value(
                                &mut vc.ssr_quality,
                                "Medium".to_string(),
                                "Medium",
                            );
                            ui.selectable_value(&mut vc.ssr_quality, "High".to_string(), "High");
                            ui.selectable_value(&mut vc.ssr_quality, "Ultra".to_string(), "Ultra");
                        });
                });
                ui.checkbox(&mut vc.ssr_temporal_upsampling, "    Temporal Upsampling");
            }
        },
    );
    if remove {
        entity.visual_correction = None;
        *is_dirty = true;
    }
}
