use std::collections::BTreeMap;

use egui_phosphor::regular as icon;
use glam::Vec3;

use crate::editor::inspector_card::component_card;
use crate::scene::{Entity, LightType, MaterialAsset};

/// 3B. Mesh details
pub fn draw_mesh(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    if entity.mesh.is_none() {
        return;
    }
    let mut remove = false;
    component_card(ui, icon::CUBE, "Mesh Filter", Some(&mut remove), |ui| {
        if let Some(mesh) = &entity.mesh {
            ui.label(format!("Type: {} primitive", mesh.primitive_type));
            ui.label(format!(
                "Geometry: {} verts | {} indices",
                mesh.vertices.len(),
                mesh.indices.len()
            ));
        }
    });
    if remove {
        entity.mesh = None;
        *is_dirty = true;
    }
}

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

        optional_map(ui, "Use Metallic Map", &mut material.metallic_map);
        optional_map(ui, "Use Roughness Map", &mut material.roughness_map);
    });
    if remove {
        *is_dirty = true;
    }
    remove
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

/// 3C. Light configuration
pub fn draw_light(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    if entity.light.is_none() {
        return;
    }
    let mut remove = false;
    component_card(ui, icon::LIGHTBULB, "Light", Some(&mut remove), |ui| {
        let Some(light) = &mut entity.light else {
            return;
        };

        draw_light_type(ui, light, is_dirty);

        let mut color_arr = [light.color.x, light.color.y, light.color.z];
        ui.horizontal(|ui| {
            ui.label("Color:");
            if ui.color_edit_button_rgb(&mut color_arr).changed() {
                light.color = Vec3::new(color_arr[0], color_arr[1], color_arr[2]);
                *is_dirty = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Intensity:");
            if ui
                .add(egui::Slider::new(&mut light.intensity, 0.0..=20.0))
                .changed()
            {
                *is_dirty = true;
            }
        });

        if light.light_type == LightType::Point || light.light_type == LightType::Spotlight {
            ui.horizontal(|ui| {
                ui.label("Distance:");
                if ui
                    .add(egui::Slider::new(&mut light.range, 0.1..=100.0))
                    .changed()
                {
                    *is_dirty = true;
                }
            });
        }

        if light.light_type == LightType::Spotlight {
            draw_spot_cones(ui, light, is_dirty);
        }
    });
    if remove {
        entity.light = None;
        *is_dirty = true;
    }
}

/// The light-type selector combo box. Switching to Spotlight seeds default cone
/// angles when they are still zero so the spot is immediately visible.
fn draw_light_type(
    ui: &mut egui::Ui,
    light: &mut crate::scene::LightComponent,
    is_dirty: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label("Type:");
        let current_type_name = match light.light_type {
            LightType::Point => "Point",
            LightType::Directional => "Directional",
            LightType::Spotlight => "Spot",
            LightType::Ambient => "Ambient",
        };
        egui::ComboBox::from_id_source("LightTypeSelector")
            .selected_text(current_type_name)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(light.light_type == LightType::Point, "Point")
                    .clicked()
                {
                    light.light_type = LightType::Point;
                    *is_dirty = true;
                }
                if ui
                    .selectable_label(light.light_type == LightType::Directional, "Directional")
                    .clicked()
                {
                    light.light_type = LightType::Directional;
                    *is_dirty = true;
                }
                if ui
                    .selectable_label(light.light_type == LightType::Spotlight, "Spot")
                    .clicked()
                {
                    light.light_type = LightType::Spotlight;
                    *is_dirty = true;
                    if light.inner_cone == 0.0 && light.outer_cone == 0.0 {
                        light.inner_cone = 30.0;
                        light.outer_cone = 45.0;
                    }
                }
            });
    });
}

/// The spotlight cone sliders: outer (FOV) and inner cone, kept ordered so the
/// inner cone never exceeds the outer one.
fn draw_spot_cones(
    ui: &mut egui::Ui,
    light: &mut crate::scene::LightComponent,
    is_dirty: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label("FOV (Outer Cone):");
        if ui
            .add(egui::Slider::new(&mut light.outer_cone, 0.1..=90.0).suffix("°"))
            .changed()
        {
            *is_dirty = true;
            if light.inner_cone > light.outer_cone {
                light.inner_cone = light.outer_cone;
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Inner Cone:");
        if ui
            .add(egui::Slider::new(&mut light.inner_cone, 0.0..=90.0).suffix("°"))
            .changed()
        {
            *is_dirty = true;
            if light.inner_cone > light.outer_cone {
                light.outer_cone = light.inner_cone;
            }
        }
    });
}
