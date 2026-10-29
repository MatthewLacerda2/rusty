//! The volume's lighting sections of the Post Processing card: Shadows (#435) and
//! Ambient Occlusion (#436), no post effects but drawn among them.
//!
//! A THIN client (#287): each widget reads the snapshot and writes through the
//! shared `authoring::rendering::post_processing` op `Graphics.*` calls too.

use super::{count_row, quality_row, slider_row, Section};
use crate::components::SsaoSettings;
use crate::core::settings::quality::{EffectQuality, MAX_RESOLUTION_DIVISOR};
use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::PostProcessingComponent;

pub(super) const SHADOWS: Section = Section {
    effect: "shadows",
    group: None,
    order: 50,
    draw: draw_shadows,
};

pub(super) const SSAO: Section = Section {
    effect: "ssao",
    group: None,
    order: 51,
    draw: draw_ssao,
};

/// The Shadows section: the sun's cascade count and reach (#435).
fn draw_shadows(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Shadows");
    let mut cascades = vc.shadows.cascades;
    let max = crate::components::ShadowSettings::MAX_CASCADES;
    ui.horizontal(|ui| {
        ui.label("  Cascades:");
        if ui
            .add(crate::editor::widgets::slider(&mut cascades, 1..=max))
            .changed()
        {
            if let Some(mut c) = world.post_processing_mut(id) {
                vc_ops::volume::set_shadow_cascades(&mut c, cascades);
            }
            *is_dirty = true;
        }
    });
    let mut distance = vc.shadows.distance;
    ui.horizontal(|ui| {
        ui.label("  Distance:");
        let drag = egui::DragValue::new(&mut distance).speed(1.0).suffix(" m");
        if ui.add(drag).changed() {
            if let Some(mut c) = world.post_processing_mut(id) {
                vc_ops::volume::set_shadow_distance(&mut c, distance);
            }
            *is_dirty = true;
        }
    });
}

/// The Ambient Occlusion section (#436): on/off, radius and strength, and its
/// quality level with the `Custom` samples and divisor (#1522).
fn draw_ssao(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Ambient Occlusion (SSAO)");
    let mut active = vc.ssao.active;
    if ui.checkbox(&mut active, "  SSAO Active").changed() {
        if let Some(mut c) = world.post_processing_mut(id) {
            vc_ops::volume::set_ssao_active(&mut c, active);
        }
        *is_dirty = true;
    }
    if vc.ssao.active {
        let (r, i) = (vc.ssao.radius, vc.ssao.intensity);
        let max = SsaoSettings::MAX_INTENSITY;
        *is_dirty |= slider_row(
            ui,
            (&mut *world, id),
            "    Radius:",
            r,
            0.05..=3.0,
            vc_ops::volume::set_ssao_radius,
        );
        *is_dirty |= slider_row(
            ui,
            (&mut *world, id),
            "    Intensity:",
            i,
            0.0..=max,
            vc_ops::volume::set_ssao_intensity,
        );
        draw_ssao_quality(ui, (world, id), vc, is_dirty);
    }
}

/// SSAO's quality level and, at `Custom`, its sample count and divisor.
fn draw_ssao_quality(
    ui: &mut egui::Ui,
    (world, id): (&mut crate::ecs::World, u32),
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    if let Some(quality) = quality_row(ui, "SSAO Quality", vc.ssao.quality) {
        if let Some(mut c) = world.post_processing_mut(id) {
            vc_ops::volume::set_ssao_quality(&mut c, quality);
        }
        *is_dirty = true;
    }
    if vc.ssao.quality != EffectQuality::Custom {
        return;
    }
    let samples = (vc.ssao.samples, 1..=SsaoSettings::MAX_SAMPLES);
    let set = vc_ops::volume::set_ssao_samples;
    *is_dirty |= count_row(
        ui,
        (&mut *world, id),
        "    Samples:",
        samples.0,
        samples.1,
        set,
    );
    let divisor = (vc.ssao.resolution_divisor, 1..=MAX_RESOLUTION_DIVISOR);
    let set = vc_ops::volume::set_ssao_resolution_divisor;
    let label = "    Resolution Divisor:";
    *is_dirty |= count_row(ui, (world, id), label, divisor.0, divisor.1, set);
}
