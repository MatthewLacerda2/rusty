//! src/editor/inspector/components/material/mod.rs — the Material inspector card.
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

mod shader_params;
mod slots;
mod surface;

use crate::editor::inspector::components::card::remove_component;
use crate::scene::authoring::ComponentKind;
use std::collections::BTreeMap;

use egui_phosphor::regular as icon;

use crate::asset::MapRef;
use crate::editor::inspector::components::card::component_card;
use crate::editor::inspector::preview::{self, PreviewSubject};
use crate::editor::EditorUi;
use crate::scene::authoring::material as mat_ops;
use crate::scene::authoring::material::FACTOR_RANGE;
use crate::scene::{MaterialAsset, RenderMode};

/// 3B2. Render the Material card for the entity `id`, editing the shared `MaterialAsset`
/// it references in the library. Folds in any pending material the Add menu staged (it
/// lacked library access); on the card's "remove" detaches the entity's reference — the
/// shared asset stays in the library for other referencing entities. No-op when the
/// entity references no material. Carries a Details/Preview tab strip (#352) — materials
/// have no on-disk file to dispatch a Preview tab on the way `assets/mod.rs` does, so it
/// hangs off this card instead.
pub fn draw_material_card(
    ui: &mut egui::Ui,
    editor: &mut EditorUi,
    world: &mut crate::ecs::World,
    id: u32,
    materials: &mut BTreeMap<String, MaterialAsset>,
) {
    let Some(first) = world.material(id).map(|m| m.first().to_string()) else {
        return;
    };
    // The Add menu's staged default fills the key only when the library has nothing
    // there: an existing asset is kept, the rule every attach path shares (#997).
    let pending = world.take_pending_material(id).unwrap_or_default();
    mat_ops::fold_pending(materials, &first, pending);
    // A multi-material mesh lists its slots; the card edits the one selected (#1285).
    let Some(key) = slots::draw_slots(ui, world, id, materials, &mut editor.is_dirty) else {
        return;
    };
    mat_ops::fold_pending(materials, &key, MaterialAsset::default());

    preview::draw_tab_strip(ui, &mut editor.preview_tab_active, editor.theme);
    if editor.preview_tab_active {
        preview::draw(
            ui,
            editor,
            PreviewSubject::Material(Box::new(materials[&key].clone())),
        );
        return;
    }

    if draw_material(ui, materials, &key, &mut editor.is_dirty) {
        remove_component(world, id, ComponentKind::Texture, &mut editor.is_dirty);
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
        surface::draw_surface_fields(ui, materials, key, is_dirty);

        // The authored surface shader (#396) — `Material.SetShader`'s twin.
        let mut shader = materials[key].shader.clone().unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label("Shader:");
            if ui.text_edit_singleline(&mut shader).changed() {
                mat_ops::set_shader(materials, key, shader);
                *is_dirty = true;
            }
        })
        .response
        .on_hover_text("An authored surface shader's name; empty = the standard shader");
        shader_params::draw_shader_params(ui, materials, key, is_dirty);
        shader_params::draw_shader_textures(ui, materials, key, is_dirty);
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
    let mut albedo = materials[key]
        .base_color_map
        .as_ref()
        .map(MapRef::to_string)
        .unwrap_or_default();
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

    if let Some(metallic) = factor_slider(ui, "Metallic:", materials[key].metallic) {
        mat_ops::set_metallic(materials, key, metallic);
        *is_dirty = true;
    }

    if let Some(roughness) = factor_slider(ui, "Roughness:", materials[key].roughness) {
        mat_ops::set_roughness(materials, key, roughness);
        *is_dirty = true;
    }

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
        egui::ComboBox::from_id_salt("RenderModeSelector")
            .selected_text(render_mode.name())
            .show_ui(ui, |ui| {
                for mode in RenderMode::ALL {
                    if ui
                        .selectable_value(&mut render_mode, mode, mode.name())
                        .changed()
                    {
                        mat_ops::set_render_mode(materials, key, render_mode);
                        *is_dirty = true;
                    }
                }
            });
    });

    if render_mode == RenderMode::Transparent {
        if let Some(alpha) = factor_slider(ui, "Alpha:", materials[key].alpha) {
            mat_ops::set_alpha(materials, key, alpha);
            *is_dirty = true;
        }
    }
    if render_mode == RenderMode::Cutout {
        if let Some(cutoff) = factor_slider(ui, "Alpha Cutoff:", materials[key].alpha_cutoff) {
            mat_ops::set_alpha_cutoff(materials, key, cutoff);
            *is_dirty = true;
        }
    }
}

/// A labelled slider over [`FACTOR_RANGE`]; the new value when the user moved it.
fn factor_slider(ui: &mut egui::Ui, label: &str, mut value: f32) -> Option<f32> {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(crate::editor::widgets::slider(&mut value, FACTOR_RANGE))
            .changed()
    })
    .inner
    .then_some(value)
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
    fn get(self, m: &MaterialAsset) -> &Option<MapRef> {
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
    if let Some(mut path) = kind.get(&materials[key]).as_ref().map(MapRef::to_string) {
        ui.horizontal(|ui| {
            ui.label("  Path:");
            if ui.text_edit_singleline(&mut path).changed() {
                kind.set(materials, key, Some(path));
            }
        });
    }
}
