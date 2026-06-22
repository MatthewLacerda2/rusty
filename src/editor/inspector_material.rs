//! src/editor/inspector_material.rs — the Material inspector card.
//!
//! Edits the shared library `MaterialAsset` an entity references (PBR factors, the
//! glTF-PBR map paths, and the transparency story — render mode + alpha + cutoff,
//! #242). Split out of `inspector_render` (mesh + light) to keep both files under the
//! size cap; the card's behaviour is unchanged.

use std::collections::BTreeMap;

use egui_phosphor::regular as icon;

use crate::editor::inspector_card::component_card;
use crate::scene::{Entity, MaterialAsset, RenderMode};

/// 3B2. Render the Material card for `entity`, editing the shared `MaterialAsset` it
/// references in the library. Folds in any `pending_material` the Add menu staged
/// (it lacked library access); on the card's "remove" detaches the entity's
/// reference — the shared asset stays in the library for other referencing entities.
/// No-op when the entity references no material.
pub fn draw_material_card(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    materials: &mut BTreeMap<String, MaterialAsset>,
    is_dirty: &mut bool,
) {
    let Some(key) = entity.material.as_ref().map(|m| m.material.clone()) else {
        return;
    };
    if let Some(pending) = entity.pending_material.take() {
        materials.insert(key.clone(), pending);
    }
    let material = materials.entry(key).or_default();
    if draw_material(ui, material, is_dirty) {
        entity.material = None;
    }
}

/// The Material card body over a resolved `MaterialAsset`. Returns `true` if the
/// user clicked the card's remove button.
fn draw_material(ui: &mut egui::Ui, material: &mut MaterialAsset, is_dirty: &mut bool) -> bool {
    let mut remove = false;
    component_card(ui, icon::PAINT_BRUSH, "Material", Some(&mut remove), |ui| {
        let mut albedo = material.base_color_map.clone().unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label("Albedo Map Path:");
            if ui.text_edit_singleline(&mut albedo).changed() {
                material.base_color_map = (!albedo.is_empty()).then_some(albedo);
                *is_dirty = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Color Tint:");
            if ui.color_edit_button_rgb(&mut material.base_color).changed() {
                *is_dirty = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Metallic:");
            ui.add(egui::Slider::new(&mut material.metallic, 0.0..=1.0));
        });

        ui.horizontal(|ui| {
            ui.label("Roughness:");
            ui.add(egui::Slider::new(&mut material.roughness, 0.0..=1.0));
        });

        ui.horizontal(|ui| {
            ui.label("Emissive:");
            if ui.color_edit_button_rgb(&mut material.emissive).changed() {
                *is_dirty = true;
            }
        });

        optional_map(ui, "Use Metallic Map", &mut material.metallic_map);
        optional_map(ui, "Use Roughness Map", &mut material.roughness_map);
        optional_map(ui, "Use Normal Map", &mut material.normal_map);
        optional_map(ui, "Use Emissive Map", &mut material.emissive_map);

        draw_material_transparency(ui, material, is_dirty);
    });
    if remove {
        *is_dirty = true;
    }
    remove
}

/// The rendering-mode selector + its mode-specific knob (#242): a `Transparent`
/// material exposes the base-color `Alpha`; a `Cutout` one exposes the `Alpha Cutoff`
/// threshold. `Opaque` shows neither — they would be inert.
fn draw_material_transparency(
    ui: &mut egui::Ui,
    material: &mut MaterialAsset,
    is_dirty: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label("Rendering Mode:");
        let label = match material.render_mode {
            RenderMode::Opaque => "Opaque",
            RenderMode::Cutout => "Cutout",
            RenderMode::Transparent => "Transparent",
        };
        egui::ComboBox::from_id_source("RenderModeSelector")
            .selected_text(label)
            .show_ui(ui, |ui| {
                for mode in [
                    RenderMode::Opaque,
                    RenderMode::Cutout,
                    RenderMode::Transparent,
                ] {
                    if ui
                        .selectable_value(&mut material.render_mode, mode, format!("{mode:?}"))
                        .changed()
                    {
                        *is_dirty = true;
                    }
                }
            });
    });

    if material.render_mode == RenderMode::Transparent {
        ui.horizontal(|ui| {
            ui.label("Alpha:");
            if ui
                .add(egui::Slider::new(&mut material.alpha, 0.0..=1.0))
                .changed()
            {
                *is_dirty = true;
            }
        });
    }
    if material.render_mode == RenderMode::Cutout {
        ui.horizontal(|ui| {
            ui.label("Alpha Cutoff:");
            if ui
                .add(egui::Slider::new(&mut material.alpha_cutoff, 0.0..=1.0))
                .changed()
            {
                *is_dirty = true;
            }
        });
    }
}

/// A "Use X Map" checkbox that, when ticked, reveals an editable map-path field.
/// Toggling the checkbox enables (empty path) or clears the optional `map`.
fn optional_map(ui: &mut egui::Ui, label: &str, map: &mut Option<String>) {
    let mut enabled = map.is_some();
    if ui.checkbox(&mut enabled, label).changed() {
        *map = enabled.then(String::new);
    }
    if let Some(map_path) = map {
        ui.horizontal(|ui| {
            ui.label("  Path:");
            ui.text_edit_singleline(map_path);
        });
    }
}
