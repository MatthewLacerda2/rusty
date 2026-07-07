use egui_phosphor::regular as icon;
use glam::Vec3;

use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::light as light_ops;
use crate::scene::{LightComponent, LightType};

/// 3B. Mesh details
pub fn draw_mesh(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    if !world.has_mesh(id) {
        return;
    }
    let mut remove = false;
    component_card(ui, icon::CUBE, "Mesh Filter", Some(&mut remove), |ui| {
        if let Some(mesh) = world.mesh(id) {
            ui.label(format!("Type: {} primitive", mesh.primitive_type));
            ui.label(format!(
                "Geometry: {} verts | {} indices",
                mesh.vertices.len(),
                mesh.indices.len()
            ));
        }
    });
    if remove {
        world.set_mesh(id, None);
        *is_dirty = true;
    }
}

/// 3C. Light configuration. A THIN client (#287): every widget reads the field into
/// a local and, on change, routes the write through the shared `authoring::light::*`
/// op — never mutating `LightComponent` fields directly. Remove detaches the
/// reference (allowed: not a field mutation).
pub fn draw_light(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(light) = world.light(id).map(|l| l.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(ui, icon::LIGHTBULB, "Light", Some(&mut remove), |ui| {
        draw_light_type(ui, world, id, &light, is_dirty);

        let mut color_arr = [light.color.x, light.color.y, light.color.z];
        ui.horizontal(|ui| {
            ui.label("Color:");
            if ui.color_edit_button_rgb(&mut color_arr).changed() {
                if let Some(mut l) = world.light_mut(id) {
                    light_ops::set_color(&mut l, Vec3::from_array(color_arr));
                }
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
                if let Some(mut l) = world.light_mut(id) {
                    light_ops::set_intensity(&mut l, intensity);
                }
                *is_dirty = true;
            }
        });

        if light.light_type == LightType::Point || light.light_type == LightType::Spotlight {
            let mut range = light.range;
            ui.horizontal(|ui| {
                ui.label("Distance:");
                if ui.add(egui::Slider::new(&mut range, 0.1..=100.0)).changed() {
                    if let Some(mut l) = world.light_mut(id) {
                        light_ops::set_range(&mut l, range);
                    }
                    *is_dirty = true;
                }
            });
        }

        if light.light_type == LightType::Spotlight {
            draw_spot_cones(ui, world, id, &light, is_dirty);
        }
    });
    if remove {
        world.set_light(id, None);
        *is_dirty = true;
    }
}

/// The light-type selector combo box. Switching to Spotlight seeds default cone
/// angles when they are still zero so the spot is immediately visible. `light` is a
/// snapshot of the current values; writes route through the shared ops.
fn draw_light_type(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
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
                    if let Some(mut l) = world.light_mut(id) {
                        light_ops::set_type(&mut l, LightType::Point);
                    }
                    *is_dirty = true;
                }
                if ui
                    .selectable_label(light.light_type == LightType::Directional, "Directional")
                    .clicked()
                {
                    if let Some(mut l) = world.light_mut(id) {
                        light_ops::set_type(&mut l, LightType::Directional);
                    }
                    *is_dirty = true;
                }
                if ui
                    .selectable_label(light.light_type == LightType::Spotlight, "Spot")
                    .clicked()
                {
                    if let Some(mut l) = world.light_mut(id) {
                        light_ops::set_type(&mut l, LightType::Spotlight);
                    }
                    *is_dirty = true;
                    if light.inner_cone == 0.0 && light.outer_cone == 0.0 {
                        if let Some(mut l) = world.light_mut(id) {
                            light_ops::set_cones(&mut l, 30.0, 45.0);
                        }
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
    world: &mut crate::ecs::World,
    id: u32,
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
            if let Some(mut l) = world.light_mut(id) {
                light_ops::set_cones(&mut l, light.inner_cone, outer);
            }
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
            if let Some(mut l) = world.light_mut(id) {
                light_ops::set_cones(&mut l, inner, outer);
            }
            *is_dirty = true;
        }
    });
}
