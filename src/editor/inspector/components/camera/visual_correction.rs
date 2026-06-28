//! src/editor/inspector/components/camera/visual_correction.rs — the Visual
//! Correction inspector card. Split out of the Camera card module to stay under the
//! size cap; the two share the camera entity (removing the Camera also drops its
//! Visual Correction volume).
//!
//! A THIN client (#287): each widget reads the field from a snapshot and routes its
//! write through the shared `authoring::visual_correction::*` op (which the
//! `Graphics.*` Lua surface calls too), never mutating the component directly.

use egui_phosphor::regular as icon;

use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::visual_correction as vc_ops;
use crate::scene::{Entity, Tonemap, VisualCorrectionComponent};

/// Visual Correction Component panel.
pub fn draw_visual_correction(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    let Some(vc) = entity.visual_correction.clone() else {
        return;
    };
    let mut remove = false;
    component_card(
        ui,
        icon::SPARKLE,
        "Visual Correction",
        Some(&mut remove),
        |ui| {
            let mut active = vc.active;
            if ui
                .checkbox(&mut active, "Enable Visual Correction")
                .changed()
            {
                vc_ops::set_active(entity, active);
                *is_dirty = true;
            }
            draw_bloom(ui, entity, &vc, is_dirty);
            draw_color_correction(ui, entity, &vc, is_dirty);
            draw_ssr(ui, entity, &vc, is_dirty);
        },
    );
    if remove {
        entity.visual_correction = None;
        *is_dirty = true;
    }
}

/// The Bloom section of the Visual Correction card.
fn draw_bloom(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    vc: &VisualCorrectionComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Bloom");
    let mut bloom_active = vc.bloom_active;
    if ui.checkbox(&mut bloom_active, "  Bloom Active").changed() {
        vc_ops::set_bloom_active(entity, bloom_active);
        *is_dirty = true;
    }
    if vc.bloom_active {
        let mut intensity = vc.bloom_intensity;
        ui.horizontal(|ui| {
            ui.label("    Intensity:");
            if ui
                .add(egui::Slider::new(&mut intensity, 0.0..=5.0))
                .changed()
            {
                vc_ops::set_bloom_intensity(entity, intensity);
                *is_dirty = true;
            }
        });
        let mut threshold = vc.bloom_threshold;
        ui.horizontal(|ui| {
            ui.label("    Threshold:");
            if ui
                .add(egui::Slider::new(&mut threshold, 0.0..=1.0))
                .changed()
            {
                vc_ops::set_bloom_threshold(entity, threshold);
                *is_dirty = true;
            }
        });
    }
}

/// The Color Correction section (exposure, contrast, saturation, tonemap, gamma).
fn draw_color_correction(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    vc: &VisualCorrectionComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Color Correction");
    let mut exposure = vc.exposure;
    ui.horizontal(|ui| {
        ui.label("  Exposure (EV):");
        if ui
            .add(egui::Slider::new(&mut exposure, -4.0..=4.0))
            .changed()
        {
            vc_ops::set_exposure(entity, exposure);
            *is_dirty = true;
        }
    });
    let mut contrast = vc.contrast;
    ui.horizontal(|ui| {
        ui.label("  Contrast:");
        if ui
            .add(egui::Slider::new(&mut contrast, 0.0..=2.0))
            .changed()
        {
            vc_ops::set_contrast(entity, contrast);
            *is_dirty = true;
        }
    });
    let mut saturation = vc.saturation;
    ui.horizontal(|ui| {
        ui.label("  Saturation:");
        if ui
            .add(egui::Slider::new(&mut saturation, 0.0..=2.0))
            .changed()
        {
            vc_ops::set_saturation(entity, saturation);
            *is_dirty = true;
        }
    });
    draw_tonemap(ui, entity, vc, is_dirty);
    let mut gamma = vc.gamma;
    ui.horizontal(|ui| {
        ui.label("  Gamma:");
        if ui.add(egui::Slider::new(&mut gamma, 1.0..=3.0)).changed() {
            vc_ops::set_gamma(entity, gamma);
            *is_dirty = true;
        }
    });
}

/// The tonemap-operator selector, reading the snapshot and routing through the op.
fn draw_tonemap(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    vc: &VisualCorrectionComponent,
    is_dirty: &mut bool,
) {
    let mut tonemap = vc.tonemap;
    ui.horizontal(|ui| {
        ui.label("  Tonemap:");
        egui::ComboBox::from_id_source("tonemap")
            .selected_text(format!("{tonemap:?}"))
            .show_ui(ui, |ui| {
                for (value, label) in [
                    (Tonemap::Aces, "ACES"),
                    (Tonemap::Reinhard, "Reinhard"),
                    (Tonemap::None, "None"),
                ] {
                    if ui.selectable_value(&mut tonemap, value, label).changed() {
                        vc_ops::set_tonemap(entity, tonemap);
                        *is_dirty = true;
                    }
                }
            });
    });
}

/// The Screen Space Reflections (SSR) section of the Visual Correction card.
fn draw_ssr(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    vc: &VisualCorrectionComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Screen Space Reflections (SSR)");
    let mut ssr_active = vc.ssr_active;
    if ui.checkbox(&mut ssr_active, "  SSR Active").changed() {
        vc_ops::set_ssr_active(entity, ssr_active);
        *is_dirty = true;
    }
    if vc.ssr_active {
        let mut quality = vc.ssr_quality.clone();
        ui.horizontal(|ui| {
            ui.label("    Quality:");
            egui::ComboBox::from_label("")
                .selected_text(&quality)
                .show_ui(ui, |ui| {
                    for q in ["Low", "Medium", "High", "Ultra"] {
                        if ui
                            .selectable_value(&mut quality, q.to_string(), q)
                            .changed()
                        {
                            vc_ops::set_ssr_quality(entity, quality.clone());
                            *is_dirty = true;
                        }
                    }
                });
        });
        let mut upsampling = vc.ssr_temporal_upsampling;
        if ui
            .checkbox(&mut upsampling, "    Temporal Upsampling")
            .changed()
        {
            vc_ops::set_ssr_temporal_upsampling(entity, upsampling);
            *is_dirty = true;
        }
    }
}
