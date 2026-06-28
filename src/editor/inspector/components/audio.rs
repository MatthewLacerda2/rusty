//! src/editor/inspector_audio.rs — the AudioSource inspector card (#212).
//!
//! Edits every serde-persisted field of `AudioSourceComponent`: the clip path, per-
//! source volume, loop / play-on-start flags, the time-scaled toggle (gameplay vs.
//! music/UI), and the spatial fields stored now for #213 (spatial blend + the
//! linear rolloff distances). Pure authoring data — the card never touches the audio
//! device; the `AudioMaestro` reads these when a voice starts.

use egui_phosphor::regular as icon;

use crate::editor::inspector::components::card::component_card;
use crate::scene::{AudioSourceComponent, Entity};

/// 3F-audio. AudioSource component card.
pub fn draw(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    if entity.audio.is_none() {
        return;
    }
    let mut remove = false;
    let mut changed = false;
    component_card(
        ui,
        icon::SPEAKER_HIGH,
        "Audio Source",
        Some(&mut remove),
        |ui| {
            let Some(a) = &mut entity.audio else {
                return;
            };
            changed |= draw_clip(ui, a);
            ui.separator();
            changed |= draw_playback(ui, a);
            ui.separator();
            changed |= draw_spatial(ui, a);
        },
    );
    if remove {
        entity.audio = None;
        *is_dirty = true;
    } else if changed {
        *is_dirty = true;
    }
}

/// The clip path + per-source volume.
fn draw_clip(ui: &mut egui::Ui, a: &mut AudioSourceComponent) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Clip:");
        changed |= ui.text_edit_singleline(&mut a.clip).changed();
    });
    changed |= clamped(ui, "Volume:", &mut a.volume, 0.0..=1.0);
    changed
}

/// Playback flags: loop, play-on-start, and the time-scaled toggle.
fn draw_playback(ui: &mut egui::Ui, a: &mut AudioSourceComponent) -> bool {
    let mut changed = ui.checkbox(&mut a.looping, "Loop").changed();
    changed |= ui.checkbox(&mut a.play_on_start, "Play On Start").changed();
    changed |= ui
        .checkbox(
            &mut a.is_time_scaled,
            "Time-Scaled (follows Time.timeScale)",
        )
        .on_hover_text("Off: wall-clock rate, for music / UI that plays through a pause.")
        .changed();
    changed
}

/// Spatial fields — stored now, consumed by #213. Shown so an author can set them
/// up ahead of the spatialization landing.
fn draw_spatial(ui: &mut egui::Ui, a: &mut AudioSourceComponent) -> bool {
    ui.label("Spatial (3D — applied by #213)");
    let mut changed = clamped(ui, "Spatial Blend:", &mut a.spatial_blend, 0.0..=1.0);
    changed |= clamped(ui, "Min Distance:", &mut a.initial_distance, 0.0..=1000.0);
    changed |= clamped(ui, "Max Distance:", &mut a.final_distance, 0.0..=1000.0);
    changed
}

/// A labelled, clamped `f32` drag row. Returns whether the value changed.
fn clamped(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(0.01).clamp_range(range))
            .changed()
    })
    .inner
}
