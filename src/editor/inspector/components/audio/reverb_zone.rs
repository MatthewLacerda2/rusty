//! src/editor/inspector/components/audio/reverb_zone.rs — the AudioReverbZone card
//! (#469).
//!
//! A THIN client (#287): widgets read a snapshot and route every write through the
//! shared `authoring::reverb_zone` ops the `AudioReverbZone.*` Lua setters call, so
//! picking a preset writes its params and editing a param makes the zone `Custom`
//! in one place.

use egui_phosphor::regular as icon;

use crate::components::reverb_zone::{DECAY_RANGE, MAX_PRE_DELAY};
use crate::components::{ReverbParams, ReverbPreset, ReverbZoneComponent as Zone};
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::reverb_zone as ops;

/// Route one edit through the shared ops and mark the scene dirty.
fn write(world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool, op: impl FnOnce(&mut Zone)) {
    if let Some(mut z) = world.reverb_zone_mut(id) {
        op(&mut z);
    }
    *is_dirty = true;
}

/// One labelled, clamped drag row; `Some(new)` when it was edited.
fn drag_row(ui: &mut egui::Ui, label: &str, mut value: f32, range: (f32, f32)) -> Option<f32> {
    ui.horizontal(|ui| {
        ui.label(label);
        let drag = egui::DragValue::new(&mut value)
            .speed(0.01)
            .range(range.0..=range.1);
        ui.add(drag).changed().then_some(value)
    })
    .inner
}

/// 3F-reverb. AudioReverbZone component card.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(z) = world.reverb_zone(id).map(|z| z.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(
        ui,
        icon::WAVEFORM,
        "Audio Reverb Zone",
        Some(&mut remove),
        |ui| {
            draw_radii(ui, world, id, &z, is_dirty);
            draw_preset(ui, world, id, &z, is_dirty);
            draw_params(ui, world, id, z.params, is_dirty);
        },
    );
    if remove {
        world.set_reverb_zone(id, None);
        *is_dirty = true;
    }
}

/// The full-effect and fade-out radii.
fn draw_radii(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, z: &Zone, d: &mut bool) {
    if let Some(v) = drag_row(ui, "Min Distance:", z.min_distance, (0.0, 1000.0)) {
        write(world, id, d, |z| ops::set_min_distance(z, v));
    }
    if let Some(v) = drag_row(ui, "Max Distance:", z.max_distance, (0.0, 1000.0)) {
        write(world, id, d, |z| ops::set_max_distance(z, v));
    }
}

/// The preset picker.
fn draw_preset(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, z: &Zone, d: &mut bool) {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.label("Preset:");
        egui::ComboBox::from_id_salt(("reverb_preset", id))
            .selected_text(z.preset.name())
            .show_ui(ui, |ui| {
                for preset in ReverbPreset::ALL {
                    let on = preset == z.preset;
                    if ui.selectable_label(on, preset.name()).clicked() && !on {
                        picked = Some(preset);
                    }
                }
            });
    });
    if let Some(preset) = picked {
        write(world, id, d, |z| ops::set_preset(z, preset));
    }
}

/// Decay, pre-delay, damping and wet level; any edit makes the zone `Custom`.
fn draw_params(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    p: ReverbParams,
    d: &mut bool,
) {
    let edited = [
        drag_row(ui, "Decay Time (s):", p.decay_time, DECAY_RANGE)
            .map(|v| ReverbParams { decay_time: v, ..p }),
        drag_row(ui, "Pre-Delay (s):", p.pre_delay, (0.0, MAX_PRE_DELAY))
            .map(|v| ReverbParams { pre_delay: v, ..p }),
        drag_row(ui, "Damping:", p.damping, (0.0, 1.0)).map(|v| ReverbParams { damping: v, ..p }),
        drag_row(ui, "Wet Level:", p.wet, (0.0, 1.0)).map(|v| ReverbParams { wet: v, ..p }),
    ];
    if let Some(params) = edited.into_iter().flatten().next() {
        write(world, id, d, |z| ops::set_params(z, params));
    }
}
