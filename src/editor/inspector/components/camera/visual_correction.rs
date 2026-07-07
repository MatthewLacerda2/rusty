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
use crate::scene::{Tonemap, VisualCorrectionComponent};

/// Visual Correction Component panel.
pub fn draw_visual_correction(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
) {
    let Some(vc) = world.visual_correction(id).map(|vc| vc.clone()) else {
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
                if let Some(mut c) = world.visual_correction_mut(id) {
                    vc_ops::set_active(&mut c, active);
                }
                *is_dirty = true;
            }
            draw_bloom(ui, world, id, &vc, is_dirty);
            draw_color_correction(ui, world, id, &vc, is_dirty);
            draw_ssr(ui, world, id, &vc, is_dirty);
        },
    );
    if remove {
        world.set_visual_correction(id, None);
        *is_dirty = true;
    }
}

/// The Bloom section of the Visual Correction card.
fn draw_bloom(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &VisualCorrectionComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Bloom");
    let mut bloom_active = vc.bloom_active;
    if ui.checkbox(&mut bloom_active, "  Bloom Active").changed() {
        if let Some(mut c) = world.visual_correction_mut(id) {
            vc_ops::set_bloom_active(&mut c, bloom_active);
        }
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
                if let Some(mut c) = world.visual_correction_mut(id) {
                    vc_ops::set_bloom_intensity(&mut c, intensity);
                }
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
                if let Some(mut c) = world.visual_correction_mut(id) {
                    vc_ops::set_bloom_threshold(&mut c, threshold);
                }
                *is_dirty = true;
            }
        });
    }
}

/// A labelled slider row that routes its write through a shared vc op (#287).
/// Returns whether the value changed; `cx` is the (world, entity-id) pair.
fn slider_row(
    ui: &mut egui::Ui,
    cx: (&mut crate::ecs::World, u32),
    label: &str,
    mut value: f32,
    range: std::ops::RangeInclusive<f32>,
    op: fn(&mut VisualCorrectionComponent, f32),
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        let changed = ui.add(egui::Slider::new(&mut value, range)).changed();
        if changed {
            if let Some(mut c) = cx.0.visual_correction_mut(cx.1) {
                op(&mut c, value);
            }
        }
        changed
    })
    .inner
}

/// The Color Correction section (exposure, contrast, saturation, tonemap, gamma).
#[rustfmt::skip]
fn draw_color_correction(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &VisualCorrectionComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Color Correction");
    *is_dirty |= slider_row(ui, (&mut *world, id), "  Exposure (EV):", vc.exposure, -4.0..=4.0, vc_ops::set_exposure);
    *is_dirty |= slider_row(ui, (&mut *world, id), "  Contrast:", vc.contrast, 0.0..=2.0, vc_ops::set_contrast);
    *is_dirty |= slider_row(ui, (&mut *world, id), "  Saturation:", vc.saturation, 0.0..=2.0, vc_ops::set_saturation);
    draw_tonemap(ui, world, id, vc, is_dirty);
    *is_dirty |= slider_row(ui, (&mut *world, id), "  Gamma:", vc.gamma, 1.0..=3.0, vc_ops::set_gamma);
}

/// The tonemap-operator selector, reading the snapshot and routing through the op.
fn draw_tonemap(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
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
                        if let Some(mut c) = world.visual_correction_mut(id) {
                            vc_ops::set_tonemap(&mut c, tonemap);
                        }
                        *is_dirty = true;
                    }
                }
            });
    });
}

/// The Screen Space Reflections (SSR) section of the Visual Correction card.
fn draw_ssr(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &VisualCorrectionComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Screen Space Reflections (SSR)");
    let mut ssr_active = vc.ssr_active;
    if ui.checkbox(&mut ssr_active, "  SSR Active").changed() {
        if let Some(mut c) = world.visual_correction_mut(id) {
            vc_ops::set_ssr_active(&mut c, ssr_active);
        }
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
                            if let Some(mut c) = world.visual_correction_mut(id) {
                                vc_ops::set_ssr_quality(&mut c, quality.clone());
                            }
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
            if let Some(mut c) = world.visual_correction_mut(id) {
                vc_ops::set_ssr_temporal_upsampling(&mut c, upsampling);
            }
            *is_dirty = true;
        }
    }
}
