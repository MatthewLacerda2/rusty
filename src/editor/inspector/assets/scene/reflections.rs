//! The scene inspector's **Reflection Probes** section (#1584): the probes' scene
//! defaults (resolution, memory budget, probe ceiling) and every probe's Unity Reflection Probe
//! settings, the same fields `Reflection.Set` / `Reflection.Get` and
//! `Lighting.SetSettings` write. Edits go through `ReflectionProbeSet::edit`, which
//! clamps them as the API's do.

use egui_phosphor::regular as icon;

use crate::editor::widgets::DragRange;
use crate::editor::EditorUi;
use crate::scene::lighting::settings::ReflectionSettings;
use crate::scene::{ReflectionProbe, Scene};

/// The face sizes the resolution menus offer: Unity's.
const SIZES: [u32; 8] = [16, 32, 64, 128, 256, 512, 1024, 2048];

pub fn draw(ui: &mut egui::Ui, editor: &mut EditorUi, scene: &mut Scene) {
    ui.add_space(8.0);
    egui::CollapsingHeader::new(format!("{}  Reflection Probes", icon::SPHERE))
        .default_open(false)
        .show(ui, |ui| {
            let mut changed = draw_defaults(ui, &mut scene.lighting_settings.reflections);
            changed |= draw_probes(ui, scene);
            editor.is_dirty |= changed;
        });
}

/// The scene-wide defaults; returns whether one changed.
fn draw_defaults(ui: &mut egui::Ui, r: &mut ReflectionSettings) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let mut size = Some(r.resolution);
        changed |= resolution_menu(ui, "default_reflection_resolution", &mut size, None);
        r.resolution = size.unwrap_or(r.resolution);
        ui.label("Default resolution");
    });
    ui.horizontal(|ui| {
        let budget = egui::DragValue::new(&mut r.budget_mb).ranged(ReflectionSettings::BUDGET_MB);
        changed |= ui
            .add(budget.suffix(" MB"))
            .on_hover_text("Probe cubes kept in memory at once, nearest boxes first")
            .changed();
        ui.label("Memory budget");
    });
    ui.horizontal(|ui| {
        let max = egui::DragValue::new(&mut r.max_probes).ranged(ReflectionSettings::MAX_PROBES);
        changed |= ui
            .add(max)
            .on_hover_text("The most probes Generate Lighting places; past it, tiles widen")
            .changed();
        ui.label("Max probes");
    });
    if changed {
        r.clamp();
    }
    changed
}

/// One collapsing card per probe, and Add; returns whether anything changed.
fn draw_probes(ui: &mut egui::Ui, scene: &mut Scene) -> bool {
    let default = scene.lighting_settings.reflections.resolution;
    let mut changed = false;
    let mut remove = None;
    for i in 0..scene.reflection_probes.len() {
        let mut probe = scene.reflection_probes.probes[i].clone();
        let title = format!("Probe {i}");
        let edited = egui::CollapsingHeader::new(title)
            .id_salt(("reflection_probe", i))
            .show(ui, |ui| {
                if ui.button(format!("{}  Remove", icon::TRASH)).clicked() {
                    remove = Some(i);
                }
                draw_probe(ui, i, &mut probe, default)
            })
            .body_returned
            .unwrap_or(false);
        if edited {
            scene.reflection_probes.edit(i, |p| *p = probe);
            changed = true;
        }
    }
    if let Some(i) = remove {
        scene.reflection_probes.remove(i);
        changed = true;
    }
    if ui.button(format!("{}  Add probe", icon::PLUS)).clicked() {
        scene
            .reflection_probes
            .probes
            .push(ReflectionProbe::default());
        changed = true;
    }
    changed
}

/// Probe `i`'s fields; returns whether one changed.
fn draw_probe(ui: &mut egui::Ui, i: usize, p: &mut ReflectionProbe, default: u32) -> bool {
    let mut changed = false;
    changed |= vec_row(ui, "Position", &mut p.position);
    changed |= vec_row(ui, "Box min", &mut p.box_min);
    changed |= vec_row(ui, "Box max", &mut p.box_max);
    changed |= ui
        .checkbox(&mut p.box_projection, "Box projection")
        .changed();
    let mut row = |ui: &mut egui::Ui, drag: egui::DragValue, label: &str| {
        ui.horizontal(|ui| {
            changed |= ui.add(drag).changed();
            ui.label(label);
        });
    };
    let blend = egui::DragValue::new(&mut p.blend_distance).ranged(0.0..=f32::MAX);
    row(ui, blend.speed(0.05), "Blend distance");
    row(ui, egui::DragValue::new(&mut p.importance), "Importance");
    let near =
        egui::DragValue::new(&mut p.near_clip).ranged(ReflectionProbe::MIN_NEAR_CLIP..=f32::MAX);
    row(ui, near.speed(0.01), "Near clip");
    let far = egui::DragValue::new(&mut p.far_clip).ranged(0.0..=f32::MAX);
    row(ui, far.speed(1.0), "Far clip");
    let intensity = egui::DragValue::new(&mut p.intensity).ranged(ReflectionProbe::INTENSITY);
    row(ui, intensity.speed(0.01), "Intensity");
    changed |= ui.checkbox(&mut p.shadows, "Shadows").changed();
    ui.horizontal(|ui| {
        changed |= resolution_menu(
            ui,
            ("reflection_resolution", i),
            &mut p.resolution,
            Some(default),
        );
        ui.label("Resolution");
    });
    let baked = match p.cubemap_path.is_empty() {
        true => "not baked yet",
        false => p.cubemap_path.as_str(),
    };
    ui.label(format!("Cubemap: {baked}"));
    changed
}

/// A labelled x/y/z row; returns whether one changed.
fn vec_row(ui: &mut egui::Ui, label: &str, v: &mut glam::Vec3) -> bool {
    ui.horizontal(|ui| {
        let mut changed = false;
        for axis in [&mut v.x, &mut v.y, &mut v.z] {
            changed |= ui.add(egui::DragValue::new(axis).speed(0.05)).changed();
        }
        ui.label(label);
        changed
    })
    .inner
}

/// A resolution menu. With a `default`, `None` is "Default (N)", the scene's.
fn resolution_menu(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    size: &mut Option<u32>,
    default: Option<u32>,
) -> bool {
    let text = |s: Option<u32>| match (s, default) {
        (Some(s), _) => s.to_string(),
        (None, Some(d)) => format!("Default ({d})"),
        (None, None) => String::new(),
    };
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .selected_text(text(*size))
        .show_ui(ui, |ui| {
            if default.is_some() {
                changed |= ui.selectable_value(size, None, text(None)).changed();
            }
            for s in SIZES {
                changed |= ui.selectable_value(size, Some(s), s.to_string()).changed();
            }
        });
    changed
}
