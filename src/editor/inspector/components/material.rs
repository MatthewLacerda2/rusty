//! src/editor/inspector_material.rs — the Material inspector card.
//!
//! Edits the shared library `MaterialAsset` an entity references (PBR factors, the
//! glTF-PBR map paths, and the transparency story — render mode + alpha + cutoff,
//! #242). Split out of `inspector_render` (mesh + light) to keep both files under the
//! size cap; the card's behaviour is unchanged.
//!
//! This is a THIN egui client (#287): every widget reads the field into a local, and
//! on change calls the matching shared `authoring::material::*` op against the library
//! plus the resolved key — never mutating `MaterialAsset` fields directly. The field
//! write and its validation live once in that shared module, which the Lua
//! `Material.*` API calls too, so the panel and the binding are siblings over one op.

use std::collections::BTreeMap;

use egui_phosphor::regular as icon;

use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::material as mat_ops;
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
    materials.entry(key.clone()).or_default();
    if draw_material(ui, materials, &key, is_dirty) {
        entity.material = None;
    }
}

/// The Material card body over the library + resolved `key`. Each widget reads the
/// current field, and on change routes the write through the shared
/// `authoring::material::*` op. Returns `true` if the user clicked the card's remove
/// button.
fn draw_material(
    ui: &mut egui::Ui,
    materials: &mut BTreeMap<String, MaterialAsset>,
    key: &str,
    is_dirty: &mut bool,
) -> bool {
    let mut remove = false;
    component_card(ui, icon::PAINT_BRUSH, "Material", Some(&mut remove), |ui| {
        draw_material_factors(ui, materials, key, is_dirty);

        optional_map(ui, "Use Metallic Map", materials, key, MapKind::Metallic);
        optional_map(ui, "Use Roughness Map", materials, key, MapKind::Roughness);
        optional_map(ui, "Use Normal Map", materials, key, MapKind::Normal);
        optional_map(ui, "Use Emissive Map", materials, key, MapKind::Emissive);

        draw_material_transparency(ui, materials, key, is_dirty);
    });
    if remove {
        *is_dirty = true;
    }
    remove
}

/// The PBR factor widgets — albedo map path, color tint, metallic, roughness, and
/// emissive — each reading its field into a local and routing the write through the
/// shared op on change. Split out of [`draw_material`] for the size cap.
fn draw_material_factors(
    ui: &mut egui::Ui,
    materials: &mut BTreeMap<String, MaterialAsset>,
    key: &str,
    is_dirty: &mut bool,
) {
    let mut albedo = materials[key].base_color_map.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Albedo Map Path:");
        if ui.text_edit_singleline(&mut albedo).changed() {
            mat_ops::set_base_color_map(materials, key, albedo);
            *is_dirty = true;
        }
    });

    let mut base_color = materials[key].base_color;
    ui.horizontal(|ui| {
        ui.label("Color Tint:");
        if ui.color_edit_button_rgb(&mut base_color).changed() {
            mat_ops::set_base_color(materials, key, base_color);
            *is_dirty = true;
        }
    });

    let mut metallic = materials[key].metallic;
    ui.horizontal(|ui| {
        ui.label("Metallic:");
        if ui
            .add(egui::Slider::new(&mut metallic, 0.0..=1.0))
            .changed()
        {
            mat_ops::set_metallic(materials, key, metallic);
            *is_dirty = true;
        }
    });

    let mut roughness = materials[key].roughness;
    ui.horizontal(|ui| {
        ui.label("Roughness:");
        if ui
            .add(egui::Slider::new(&mut roughness, 0.0..=1.0))
            .changed()
        {
            mat_ops::set_roughness(materials, key, roughness);
            *is_dirty = true;
        }
    });

    let mut emissive = materials[key].emissive;
    ui.horizontal(|ui| {
        ui.label("Emissive:");
        if ui.color_edit_button_rgb(&mut emissive).changed() {
            mat_ops::set_emissive(materials, key, emissive);
            *is_dirty = true;
        }
    });
}

/// The rendering-mode selector + its mode-specific knob (#242): a `Transparent`
/// material exposes the base-color `Alpha`; a `Cutout` one exposes the `Alpha Cutoff`
/// threshold. `Opaque` shows neither — they would be inert.
fn draw_material_transparency(
    ui: &mut egui::Ui,
    materials: &mut BTreeMap<String, MaterialAsset>,
    key: &str,
    is_dirty: &mut bool,
) {
    let mut render_mode = materials[key].render_mode;
    ui.horizontal(|ui| {
        ui.label("Rendering Mode:");
        let label = match render_mode {
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
                        .selectable_value(&mut render_mode, mode, format!("{mode:?}"))
                        .changed()
                    {
                        mat_ops::set_render_mode(materials, key, render_mode);
                        *is_dirty = true;
                    }
                }
            });
    });

    if render_mode == RenderMode::Transparent {
        let mut alpha = materials[key].alpha;
        ui.horizontal(|ui| {
            ui.label("Alpha:");
            if ui.add(egui::Slider::new(&mut alpha, 0.0..=1.0)).changed() {
                mat_ops::set_alpha(materials, key, alpha);
                *is_dirty = true;
            }
        });
    }
    if render_mode == RenderMode::Cutout {
        let mut cutoff = materials[key].alpha_cutoff;
        ui.horizontal(|ui| {
            ui.label("Alpha Cutoff:");
            if ui.add(egui::Slider::new(&mut cutoff, 0.0..=1.0)).changed() {
                mat_ops::set_alpha_cutoff(materials, key, cutoff);
                *is_dirty = true;
            }
        });
    }
}

/// Which optional map a checkbox controls — selects the shared setter + the field to
/// read the current path from.
#[derive(Clone, Copy)]
enum MapKind {
    Metallic,
    Roughness,
    Normal,
    Emissive,
}

impl MapKind {
    /// Read this map's current path from the asset.
    fn get(self, m: &MaterialAsset) -> &Option<String> {
        match self {
            MapKind::Metallic => &m.metallic_map,
            MapKind::Roughness => &m.roughness_map,
            MapKind::Normal => &m.normal_map,
            MapKind::Emissive => &m.emissive_map,
        }
    }

    /// Route a new path (or `None`) through the matching shared op.
    fn set(self, materials: &mut BTreeMap<String, MaterialAsset>, key: &str, path: Option<String>) {
        match self {
            MapKind::Metallic => mat_ops::set_metallic_map(materials, key, path),
            MapKind::Roughness => mat_ops::set_roughness_map(materials, key, path),
            MapKind::Normal => mat_ops::set_normal_map(materials, key, path),
            MapKind::Emissive => mat_ops::set_emissive_map(materials, key, path),
        }
    }
}

/// A "Use X Map" checkbox that, when ticked, reveals an editable map-path field.
/// Toggling the checkbox enables (empty path) or clears the optional map; edits route
/// through the shared op.
fn optional_map(
    ui: &mut egui::Ui,
    label: &str,
    materials: &mut BTreeMap<String, MaterialAsset>,
    key: &str,
    kind: MapKind,
) {
    let current = kind.get(&materials[key]).clone();
    let mut enabled = current.is_some();
    if ui.checkbox(&mut enabled, label).changed() {
        kind.set(materials, key, enabled.then(String::new));
    }
    if let Some(mut path) = kind.get(&materials[key]).clone() {
        ui.horizontal(|ui| {
            ui.label("  Path:");
            if ui.text_edit_singleline(&mut path).changed() {
                kind.set(materials, key, Some(path));
            }
        });
    }
}
