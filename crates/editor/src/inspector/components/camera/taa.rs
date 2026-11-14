//! The Camera card's TAA widgets (#1702; its knobs #1847): the toggle, and, while
//! TAA is on, URP's camera TAA settings — the quality level, Base Blend Factor,
//! Variance Clamp Scale, Jitter Scale, Mip Bias and Contrast Adaptive Sharpening.
//! Writes go through the shared camera ops, so this card, `Graphics.SetTaa*` and a
//! `Graphics.taa.<knob>` tween are one path. TAA replaces FXAA while it runs.

use crate::inspector::components::particles::widgets::combo;
use rusty_components::{CameraComponent, TaaQuality};
use rusty_scene::authoring::rendering::camera as camera_ops;

pub(super) fn draw_taa(
    ui: &mut egui::Ui,
    world: &mut rusty_ecs::World,
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
    if !cam.taa_active {
        return;
    }
    draw_quality(ui, (world, id), cam, is_dirty);
    for knob in camera_ops::TAA_KNOBS {
        let mut value = (knob.get)(cam);
        let (lo, hi) = knob.range;
        if slider(ui, knob.label, &mut value, lo..=hi) {
            if let Some(mut c) = world.camera_mut(id) {
                (knob.set)(&mut c, value);
            }
            *is_dirty = true;
        }
    }
}

/// The quality dropdown (URP's `VeryLow`…`VeryHigh`).
fn draw_quality(
    ui: &mut egui::Ui,
    (world, id): (&mut rusty_ecs::World, u32),
    cam: &CameraComponent,
    is_dirty: &mut bool,
) {
    let options = TaaQuality::ALL.map(|q| (q, q.name()));
    let mut quality = cam.taa_quality;
    if combo(ui, "TAA Quality", &mut quality, &options) {
        if let Some(mut c) = world.camera_mut(id) {
            camera_ops::set_taa_quality(&mut c, quality);
        }
        *is_dirty = true;
    }
}

/// One labelled slider row; whether it changed.
fn slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(crate::widgets::slider(value, range)).changed()
    })
    .inner
}
