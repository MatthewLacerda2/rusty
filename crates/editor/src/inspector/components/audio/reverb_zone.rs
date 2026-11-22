//! A THIN client: widgets read a snapshot and route every write through the
//! shared `authoring::audio::reverb_zone` ops the `AudioReverbZone.*` Lua setters call, so
//! picking a preset writes its params and editing a param makes the zone `Custom`
//! in one place.

use crate::inspector::components::card::remove_component;
use crate::widgets::DragRange;
use egui_phosphor::regular as icon;
use rusty_edit::authoring::ComponentKind;

use crate::inspector::components::card::component_card;
use rusty_components::audio::reverb_zone::{DECAY_RANGE, MAX_PRE_DELAY};
use rusty_components::{ReverbParams, ReverbPreset, ReverbZoneComponent as Zone};
use rusty_edit::authoring::audio::reverb_zone as ops;

/// Route one edit through the shared ops and mark the scene dirty.
fn write(world: &mut rusty_ecs::World, id: u32, op: impl FnOnce(&mut Zone)) {
    if let Some(mut z) = world.reverb_zone_mut(id) {
        op(&mut z);
    }
}

/// One labelled, clamped drag row; `Some(new)` when it was edited.
fn drag_row(ui: &mut egui::Ui, label: &str, mut value: f32, range: (f32, f32)) -> Option<f32> {
    ui.horizontal(|ui| {
        ui.label(label);
        let drag = egui::DragValue::new(&mut value)
            .speed(0.01)
            .ranged(range.0..=range.1);
        ui.add(drag).changed().then_some(value)
    })
    .inner
}

pub fn draw(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32) {
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
            draw_radii(ui, world, id, &z);
            draw_preset(ui, world, id, &z);
            draw_params(ui, world, id, z.params);
        },
    );
    if remove {
        remove_component(world, id, ComponentKind::ReverbZone);
    }
}

/// The full-effect and fade-out radii.
fn draw_radii(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, z: &Zone) {
    if let Some(v) = drag_row(ui, "Min Distance:", z.min_distance, (0.0, 1000.0)) {
        write(world, id, |z| ops::set_min_distance(z, v));
    }
    if let Some(v) = drag_row(ui, "Max Distance:", z.max_distance, (0.0, 1000.0)) {
        write(world, id, |z| ops::set_max_distance(z, v));
    }
}

/// The preset picker.
fn draw_preset(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, z: &Zone) {
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
        write(world, id, |z| ops::set_preset(z, preset));
    }
}

/// Decay, pre-delay, damping and wet level; any edit makes the zone `Custom`.
fn draw_params(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, p: ReverbParams) {
    let edited = [
        drag_row(ui, "Decay Time (s):", p.decay_time, DECAY_RANGE)
            .map(|v| ReverbParams { decay_time: v, ..p }),
        drag_row(ui, "Pre-Delay (s):", p.pre_delay, (0.0, MAX_PRE_DELAY))
            .map(|v| ReverbParams { pre_delay: v, ..p }),
        drag_row(ui, "Damping:", p.damping, (0.0, 1.0)).map(|v| ReverbParams { damping: v, ..p }),
        drag_row(ui, "Wet Level:", p.wet, (0.0, 1.0)).map(|v| ReverbParams { wet: v, ..p }),
    ];
    if let Some(params) = edited.into_iter().flatten().next() {
        write(world, id, |z| ops::set_params(z, params));
    }
}

impl crate::inspector::components::add::AddEntry for rusty_components::ReverbZoneComponent {
    const CATEGORY: crate::inspector::components::add::Category =
        crate::inspector::components::add::Category::Audio;
    const LABEL: &'static str = "Audio Reverb Zone";
    const ICON: &'static str = egui_phosphor::regular::WAVEFORM;
}
