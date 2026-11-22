mod detail;
mod library;
mod maps;
mod shader_params;
mod slots;
mod surface;

pub(in crate::inspector) use library::apply as apply_action;
pub use library::{MaterialAction, MaterialDelete};

use crate::inspector::components::card::remove_component;
use rusty_edit::authoring::ComponentKind;
use std::collections::BTreeMap;

use egui_phosphor::regular as icon;

use crate::inspector::components::card::component_card;
use crate::inspector::preview::{self, PreviewSubject};
use crate::EditorUi;
use rusty_asset::MapRef;
use rusty_edit::authoring::material::FACTOR_RANGE;
use rusty_edit::authoring::material::{
    self as mat_ops, COORDINATE_FIELDS, FACE_FIELDS, SURFACE_FIELDS,
};
use rusty_scene::{MaterialAsset, RenderMode};

/// Render the Material card for the entity `id`, editing the shared library
/// `MaterialAsset` it references.
pub fn draw_material_card(
    ui: &mut egui::Ui,
    editor: &mut EditorUi,
    world: &mut rusty_ecs::World,
    id: u32,
    materials: &mut BTreeMap<String, MaterialAsset>,
    action: &mut Option<MaterialAction>,
) {
    let Some(first) = world.material(id).map(|m| m.first().to_string()) else {
        return;
    };
    let pending = world.take_pending_material(id).unwrap_or_default();
    mat_ops::fold_pending(materials, &first, pending);
    // A multi-material mesh lists its slots; the card edits the one selected.
    let Some(key) = slots::draw_slots(ui, world, id, materials, &mut editor.is_dirty) else {
        return;
    };
    mat_ops::fold_pending(materials, &key, MaterialAsset::default());
    library::draw(ui, &editor.material_delete, &key, action);

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

        maps::draw(ui, materials, key);
        surface::draw_fields(ui, COORDINATE_FIELDS, materials, key, is_dirty);
        detail::draw(ui, materials, key, is_dirty);

        draw_material_transparency(ui, materials, key, is_dirty);
        surface::draw_fields(ui, FACE_FIELDS, materials, key, is_dirty);
        surface::draw_fields(ui, SURFACE_FIELDS, materials, key, is_dirty);

        // The authored surface shader — `Material.SetShader`'s twin.
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

/// The rendering-mode selector + its mode-specific knob. `Opaque` shows neither
/// `Alpha` nor `Alpha Cutoff`: both would be inert.
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
        ui.add(crate::widgets::slider(&mut value, FACTOR_RANGE))
            .changed()
    })
    .inner
    .then_some(value)
}

impl crate::inspector::components::add::AddEntry for rusty_components::MaterialComponent {
    const CATEGORY: crate::inspector::components::add::Category =
        crate::inspector::components::add::Category::Rendering;
    const LABEL: &'static str = "Material / Texture Component";
}
