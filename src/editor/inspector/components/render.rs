use egui_phosphor::regular as icon;
use glam::Vec3;

use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::light as light_ops;
use crate::scene::{Entity, LightComponent, LightType};

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

/// 3C. Light configuration. A THIN client (#287): every widget reads the field into
/// a local and, on change, routes the write through the shared `authoring::light::*`
/// op — never mutating `LightComponent` fields directly. Remove detaches the
/// reference (allowed: not a field mutation).
pub fn draw_light(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    let Some(light) = entity.light.clone() else {
        return;
    };
    let mut remove = false;
    component_card(ui, icon::LIGHTBULB, "Light", Some(&mut remove), |ui| {
        draw_light_type(ui, entity, &light, is_dirty);

        let mut color_arr = [light.color.x, light.color.y, light.color.z];
        ui.horizontal(|ui| {
            ui.label("Color:");
            if ui.color_edit_button_rgb(&mut color_arr).changed() {
                light_ops::set_color(entity, Vec3::from_array(color_arr));
                *is_dirty = true;
            }
        });

        let mut intensity = light.intensity;
        ui.horizontal(|ui| {
            ui.label("Intensity:");
            if ui
                .add(egui::Slider::new(&mut intensity, 0.0..=20.0))
                .changed()
            {
                light_ops::set_intensity(entity, intensity);
                *is_dirty = true;
            }
        });

        if light.light_type == LightType::Point || light.light_type == LightType::Spotlight {
            let mut range = light.range;
            ui.horizontal(|ui| {
                ui.label("Distance:");
                if ui.add(egui::Slider::new(&mut range, 0.1..=100.0)).changed() {
                    light_ops::set_range(entity, range);
                    *is_dirty = true;
                }
            });
        }

        if light.light_type == LightType::Spotlight {
            draw_spot_cones(ui, entity, &light, is_dirty);
        }
    });
    if remove {
        entity.light = None;
        *is_dirty = true;
    }
}

/// The light-type selector combo box. Switching to Spotlight seeds default cone
/// angles when they are still zero so the spot is immediately visible. `light` is a
/// snapshot of the current values; writes route through the shared ops.
fn draw_light_type(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    light: &LightComponent,
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
                    light_ops::set_type(entity, LightType::Point);
                    *is_dirty = true;
                }
                if ui
                    .selectable_label(light.light_type == LightType::Directional, "Directional")
                    .clicked()
                {
                    light_ops::set_type(entity, LightType::Directional);
                    *is_dirty = true;
                }
                if ui
                    .selectable_label(light.light_type == LightType::Spotlight, "Spot")
                    .clicked()
                {
                    light_ops::set_type(entity, LightType::Spotlight);
                    *is_dirty = true;
                    if light.inner_cone == 0.0 && light.outer_cone == 0.0 {
                        light_ops::set_cones(entity, 30.0, 45.0);
                    }
                }
            });
    });
}

/// The spotlight cone sliders: outer (FOV) and inner cone. The shared op keeps the
/// inner cone from exceeding the outer one; here we read both from the snapshot,
/// edit a local, and route the pair through `set_cones`.
fn draw_spot_cones(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    light: &LightComponent,
    is_dirty: &mut bool,
) {
    let mut outer = light.outer_cone;
    ui.horizontal(|ui| {
        ui.label("FOV (Outer Cone):");
        if ui
            .add(egui::Slider::new(&mut outer, 0.1..=90.0).suffix("°"))
            .changed()
        {
            // Outer shrank below inner ⇒ pull inner down with it (op enforces it).
            light_ops::set_cones(entity, light.inner_cone, outer);
            *is_dirty = true;
        }
    });
    let mut inner = light.inner_cone;
    ui.horizontal(|ui| {
        ui.label("Inner Cone:");
        if ui
            .add(egui::Slider::new(&mut inner, 0.0..=90.0).suffix("°"))
            .changed()
        {
            // Inner grew past outer ⇒ raise outer to match (mirrors the old card).
            let outer = light.outer_cone.max(inner);
            light_ops::set_cones(entity, inner, outer);
            *is_dirty = true;
        }
    });
}
