//! src/editor/inspector/components/camera/post_processing.rs — the Post
//! Processing inspector card. Split out of the Camera card module to stay under the
//! size cap; the two share the camera entity (removing the Camera also drops its
//! Post Processing volume).
//!
//! A THIN client (#287): each widget reads the field from a snapshot and routes its
//! write through the shared `authoring::rendering::post_processing::*` op (which the
//! `Graphics.*` Lua surface calls too), never mutating the component directly.

use crate::editor::inspector::components::card::remove_component;
use crate::scene::authoring::ComponentKind;
use egui_phosphor::regular as icon;

use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::{PostProcessingComponent, Tonemap};

/// Post Processing Component panel.
pub fn draw_post_processing(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
) {
    let Some(vc) = world.post_processing(id).map(|vc| vc.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(
        ui,
        icon::SPARKLE,
        "Post Processing",
        Some(&mut remove),
        |ui| {
            let mut active = vc.active;
            if ui.checkbox(&mut active, "Enable Post Processing").changed() {
                if let Some(mut c) = world.post_processing_mut(id) {
                    vc_ops::set_active(&mut c, active);
                }
                *is_dirty = true;
            }
            draw_bloom(ui, world, id, &vc, is_dirty);
            draw_color_correction(ui, world, id, &vc, is_dirty);
            super::depth_of_field::draw_depth_of_field(ui, world, id, &vc, is_dirty);
            draw_ssr(ui, world, id, &vc, is_dirty);
            super::lighting::draw_shadows(ui, world, id, &vc, is_dirty);
            super::lighting::draw_ssao(ui, world, id, &vc, is_dirty);
            super::lens::draw_lens(ui, world, id, &vc, is_dirty);
            super::custom_effects::draw_custom_effects(ui, world, id, &vc, is_dirty);
        },
    );
    if remove {
        remove_component(world, id, ComponentKind::PostProcessing, is_dirty);
    }
}

/// The Bloom section of the Post Processing card.
fn draw_bloom(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Bloom");
    let mut bloom_active = vc.bloom.active;
    if ui.checkbox(&mut bloom_active, "  Bloom Active").changed() {
        if let Some(mut c) = world.post_processing_mut(id) {
            vc_ops::bloom::set_active(&mut c, bloom_active);
        }
        *is_dirty = true;
    }
    if vc.bloom.active {
        let mut intensity = vc.bloom.intensity;
        ui.horizontal(|ui| {
            ui.label("    Intensity:");
            if ui
                .add(crate::editor::widgets::slider(&mut intensity, 0.0..=5.0))
                .changed()
            {
                if let Some(mut c) = world.post_processing_mut(id) {
                    vc_ops::bloom::set_intensity(&mut c, intensity);
                }
                *is_dirty = true;
            }
        });
        let mut threshold = vc.bloom.threshold;
        ui.horizontal(|ui| {
            ui.label("    Threshold:");
            if ui
                .add(crate::editor::widgets::slider(&mut threshold, 0.0..=1.0))
                .changed()
            {
                if let Some(mut c) = world.post_processing_mut(id) {
                    vc_ops::bloom::set_threshold(&mut c, threshold);
                }
                *is_dirty = true;
            }
        });
    }
}

/// A labelled slider row that routes its write through a shared vc op (#287).
/// Returns whether the value changed; `cx` is the (world, entity-id) pair.
pub(super) fn slider_row(
    ui: &mut egui::Ui,
    cx: (&mut crate::ecs::World, u32),
    label: &str,
    mut value: f32,
    range: std::ops::RangeInclusive<f32>,
    op: fn(&mut PostProcessingComponent, f32),
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        let changed = ui
            .add(crate::editor::widgets::slider(&mut value, range))
            .changed();
        if changed {
            if let Some(mut c) = cx.0.post_processing_mut(cx.1) {
                op(&mut c, value);
            }
        }
        changed
    })
    .inner
}

/// The Color Correction section (exposure, contrast, saturation, grade, tonemap, gamma, LUT).
#[rustfmt::skip]
fn draw_color_correction(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Color Correction");
    super::exposure::draw_exposure(ui, world, id, vc, is_dirty);
    *is_dirty |= slider_row(ui, (&mut *world, id), "  Contrast:", vc.color_adjustments.contrast, 0.0..=2.0, vc_ops::grading::color_adjustments::set_contrast);
    *is_dirty |= slider_row(ui, (&mut *world, id), "  Saturation:", vc.color_adjustments.saturation, 0.0..=2.0, vc_ops::grading::color_adjustments::set_saturation);
    super::grading::draw_grading(ui, world, id, vc, is_dirty);
    draw_tonemap(ui, world, id, vc, is_dirty);
    *is_dirty |= slider_row(ui, (&mut *world, id), "  Display Gamma:", vc.tonemapping.display_gamma, 0.5..=2.0, vc_ops::grading::tonemapping::set_display_gamma);
    super::grading::draw_color_lookup(ui, world, id, vc, is_dirty);
}

/// The tonemap-operator selector, reading the snapshot and routing through the op.
fn draw_tonemap(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let mut tonemap = vc.tonemapping.mode;
    ui.horizontal(|ui| {
        ui.label("  Tonemap:");
        egui::ComboBox::from_id_salt("tonemap")
            .selected_text(tonemap.name())
            .show_ui(ui, |ui| {
                for value in Tonemap::ALL {
                    if ui
                        .selectable_value(&mut tonemap, value, value.name())
                        .changed()
                    {
                        if let Some(mut c) = world.post_processing_mut(id) {
                            vc_ops::grading::tonemapping::set_mode(&mut c, tonemap);
                        }
                        *is_dirty = true;
                    }
                }
            });
    });
}

/// The Screen Space Reflections (SSR) section of the Post Processing card.
fn draw_ssr(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Screen Space Reflections (SSR)");
    let mut ssr_active = vc.ssr.active;
    if ui.checkbox(&mut ssr_active, "  SSR Active").changed() {
        if let Some(mut c) = world.post_processing_mut(id) {
            vc_ops::ssr::set_active(&mut c, ssr_active);
        }
        *is_dirty = true;
    }
    if vc.ssr.active {
        let mut quality = vc.ssr.quality.clone();
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
                            if let Some(mut c) = world.post_processing_mut(id) {
                                vc_ops::ssr::set_quality(&mut c, quality.clone());
                            }
                            *is_dirty = true;
                        }
                    }
                });
        });
        let mut upsampling = vc.ssr.temporal_upsampling;
        if ui
            .checkbox(&mut upsampling, "    Temporal Upsampling")
            .changed()
        {
            if let Some(mut c) = world.post_processing_mut(id) {
                vc_ops::ssr::set_temporal_upsampling(&mut c, upsampling);
            }
            *is_dirty = true;
        }
    }
}
