//! src/editor/inspector_audio.rs — the AudioSource inspector card (#212).
//!
//! Edits every serde-persisted field of `AudioSourceComponent`: the clip path, per-
//! source volume, loop / play-on-start flags, the time-scaled toggle (gameplay vs.
//! music/UI), and the spatial fields stored now for #213 (spatial blend + the
//! linear rolloff distances). Pure authoring data — the card never touches the audio
//! device; the `AudioMaestro` reads these when a voice starts.

use egui_phosphor::regular as icon;

use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::audio as audio_ops;
use crate::scene::AudioSourceComponent;

/// 3F-audio. AudioSource component card. A THIN client (#287): each widget reads the
/// field from a snapshot and routes its write through a shared `authoring::audio::*`
/// op (no mutable component borrow); remove detaches the component.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(a) = world.audio(id).map(|a| a.clone()) else {
        return;
    };
    let mut remove = false;
    let mut changed = false;
    component_card(
        ui,
        icon::SPEAKER_HIGH,
        "Audio Source",
        Some(&mut remove),
        |ui| {
            changed |= draw_clip(ui, world, id, &a);
            ui.separator();
            changed |= draw_playback(ui, world, id, &a);
            ui.separator();
            changed |= draw_spatial(ui, world, id, &a);
        },
    );
    if remove {
        world.set_audio(id, None);
        *is_dirty = true;
    } else if changed {
        *is_dirty = true;
    }
}

/// The clip path + per-source volume.
fn draw_clip(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    a: &AudioSourceComponent,
) -> bool {
    let mut changed = false;
    let mut clip = a.clip.clone();
    ui.horizontal(|ui| {
        ui.label("Clip:");
        if ui.text_edit_singleline(&mut clip).changed() {
            if let Some(mut c) = world.audio_mut(id) {
                audio_ops::set_clip(&mut c, clip);
            }
            changed = true;
        }
    });
    let mut volume = a.volume;
    if clamped(ui, "Volume:", &mut volume, 0.0..=1.0) {
        if let Some(mut c) = world.audio_mut(id) {
            audio_ops::set_volume(&mut c, volume);
        }
        changed = true;
    }
    changed
}

/// Playback flags: loop, play-on-start, and the time-scaled toggle.
fn draw_playback(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    a: &AudioSourceComponent,
) -> bool {
    let mut changed = false;
    let mut looping = a.looping;
    if ui.checkbox(&mut looping, "Loop").changed() {
        if let Some(mut c) = world.audio_mut(id) {
            audio_ops::set_looping(&mut c, looping);
        }
        changed = true;
    }
    let mut play_on_start = a.play_on_start;
    if ui.checkbox(&mut play_on_start, "Play On Start").changed() {
        if let Some(mut c) = world.audio_mut(id) {
            audio_ops::set_play_on_start(&mut c, play_on_start);
        }
        changed = true;
    }
    let mut is_time_scaled = a.is_time_scaled;
    if ui
        .checkbox(&mut is_time_scaled, "Time-Scaled (follows Time.timeScale)")
        .on_hover_text("Off: wall-clock rate, for music / UI that plays through a pause.")
        .changed()
    {
        if let Some(mut c) = world.audio_mut(id) {
            audio_ops::set_time_scaled(&mut c, is_time_scaled);
        }
        changed = true;
    }
    changed
}

/// Spatial fields — stored now, consumed by #213. Shown so an author can set them
/// up ahead of the spatialization landing.
fn draw_spatial(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    a: &AudioSourceComponent,
) -> bool {
    ui.label("Spatial (3D — applied by #213)");
    let mut changed = false;
    let mut spatial_blend = a.spatial_blend;
    if clamped(ui, "Spatial Blend:", &mut spatial_blend, 0.0..=1.0) {
        if let Some(mut c) = world.audio_mut(id) {
            audio_ops::set_spatial_blend(&mut c, spatial_blend);
        }
        changed = true;
    }
    let mut initial_distance = a.initial_distance;
    if clamped(ui, "Min Distance:", &mut initial_distance, 0.0..=1000.0) {
        if let Some(mut c) = world.audio_mut(id) {
            audio_ops::set_initial_distance(&mut c, initial_distance);
        }
        changed = true;
    }
    let mut final_distance = a.final_distance;
    if clamped(ui, "Max Distance:", &mut final_distance, 0.0..=1000.0) {
        if let Some(mut c) = world.audio_mut(id) {
            audio_ops::set_final_distance(&mut c, final_distance);
        }
        changed = true;
    }
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
