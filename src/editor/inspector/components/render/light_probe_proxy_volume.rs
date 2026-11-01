//! src/editor/inspector/components/render/light_probe_proxy_volume.rs — the Light
//! Probe Proxy Volume card (#1672).
//!
//! A THIN client (#287): widgets read a snapshot and route every write through the
//! shared `authoring::rendering::light_probe_proxy_volume` ops the
//! `LightProbeProxyVolume.*` Lua setters call. Unity's layout: the bounding box
//! mode (with the custom size and origin under `Custom`), the resolution mode
//! (density, or a count per axis), and the quality.

use egui_phosphor::regular as icon;
use glam::Vec3;

use crate::components::rendering::light_probe_proxy_volume::{DENSITY_RANGE, MAX_RESOLUTION};
use crate::components::{
    LightProbeProxyVolumeComponent as Volume, ProxyBoundsMode, ProxyQuality, ProxyResolutionMode,
};
use crate::ecs::World;
use crate::editor::inspector::components::card::{component_card, remove_component};
use crate::editor::widgets::DragRange;
use crate::scene::authoring::rendering::light_probe_proxy_volume as ops;
use crate::scene::authoring::ComponentKind;

/// Route one edit through the shared ops and mark the scene dirty.
fn write(world: &mut World, id: u32, is_dirty: &mut bool, op: impl FnOnce(&mut Volume)) {
    if let Some(mut v) = world.light_probe_proxy_volume_mut(id) {
        op(&mut v);
    }
    *is_dirty = true;
}

/// 3CP. Light Probe Proxy Volume card.
pub fn draw_light_probe_proxy_volume(ui: &mut egui::Ui, world: &mut World, id: u32, d: &mut bool) {
    let Some(v) = world.light_probe_proxy_volume(id).map(|v| v.clone()) else {
        return;
    };
    let mut remove = false;
    let title = "Light Probe Proxy Volume";
    component_card(ui, icon::CUBE_TRANSPARENT, title, Some(&mut remove), |ui| {
        draw_bounds(ui, world, id, &v, d);
        draw_resolution(ui, world, id, &v, d);
        if let Some(q) = pick(
            ui,
            ("lppv_quality", id),
            "Quality:",
            v.quality,
            ProxyQuality::ALL,
        ) {
            write(world, id, d, |v| ops::set_quality(v, q));
        }
    });
    if remove {
        remove_component(world, id, ComponentKind::LightProbeProxyVolume, d);
    }
}

/// The bounding box mode, and the custom box under `Custom`.
fn draw_bounds(ui: &mut egui::Ui, world: &mut World, id: u32, v: &Volume, d: &mut bool) {
    let modes = ProxyBoundsMode::ALL;
    if let Some(m) = pick(
        ui,
        ("lppv_bounds", id),
        "Bounding Box:",
        v.bounds_mode,
        modes,
    ) {
        write(world, id, d, |v| ops::set_bounds_mode(v, m));
    }
    if v.bounds_mode != ProxyBoundsMode::Custom {
        return;
    }
    if let Some(size) = vec3_row(ui, "Size:", v.size) {
        write(world, id, d, |v| ops::set_size(v, size));
    }
    if let Some(origin) = vec3_row(ui, "Origin:", v.origin) {
        write(world, id, d, |v| ops::set_origin(v, origin));
    }
}

/// The resolution mode, and its density or per-axis counts.
fn draw_resolution(ui: &mut egui::Ui, world: &mut World, id: u32, v: &Volume, d: &mut bool) {
    let modes = ProxyResolutionMode::ALL;
    let salt = ("lppv_resolution", id);
    if let Some(m) = pick(ui, salt, "Resolution Mode:", v.resolution_mode, modes) {
        write(world, id, d, |v| ops::set_resolution_mode(v, m));
    }
    if v.resolution_mode == ProxyResolutionMode::Automatic {
        let mut density = v.probe_density;
        let changed = ui
            .horizontal(|ui| {
                ui.label("Density (per m):");
                let range = DENSITY_RANGE.0..=DENSITY_RANGE.1;
                let drag = egui::DragValue::new(&mut density).speed(0.05).ranged(range);
                ui.add(drag).changed()
            })
            .inner;
        if changed {
            write(world, id, d, |v| ops::set_probe_density(v, density));
        }
        return;
    }
    let mut dims = v.grid_resolution;
    let changed = ui
        .horizontal(|ui| {
            ui.label("Resolution X/Y/Z:");
            dims.iter_mut().fold(false, |c, n| {
                let drag = egui::DragValue::new(n).ranged(1..=MAX_RESOLUTION);
                ui.add(drag).changed() | c
            })
        })
        .inner;
    if changed {
        write(world, id, d, |v| ops::set_grid_resolution(v, dims));
    }
}

/// A labelled combo over a named enum's variants; `Some(new)` when one was picked.
fn pick<T: Copy + PartialEq + std::fmt::Display, const N: usize>(
    ui: &mut egui::Ui,
    salt: (&str, u32),
    label: &str,
    current: T,
    all: [T; N],
) -> Option<T> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(salt)
            .selected_text(current.to_string())
            .show_ui(ui, |ui| {
                for option in all {
                    let on = option == current;
                    if ui.selectable_label(on, option.to_string()).clicked() && !on {
                        picked = Some(option);
                    }
                }
            });
    });
    picked
}

/// A labelled x/y/z drag row; `Some(new)` when any axis changed.
fn vec3_row(ui: &mut egui::Ui, label: &str, mut value: Vec3) -> Option<Vec3> {
    let changed = ui
        .horizontal(|ui| {
            ui.label(label);
            [&mut value.x, &mut value.y, &mut value.z]
                .into_iter()
                .fold(false, |c, axis| {
                    ui.add(egui::DragValue::new(axis).speed(0.05)).changed() | c
                })
        })
        .inner;
    changed.then_some(value)
}
