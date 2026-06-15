use glam::Vec3;

use crate::scene::{Entity, LightType};

/// 3B. Mesh details
pub fn draw_mesh(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    let mut remove_mesh = false;
    if let Some(mesh) = &entity.mesh {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("📦 Mesh Filter");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🗑").on_hover_text("Remove Mesh Filter").clicked() {
                    remove_mesh = true;
                }
            });
        });
        ui.label(format!("Type: {} primitive", mesh.primitive_type));
        ui.label(format!(
            "Geometry: {} verts | {} indices",
            mesh.vertices.len(),
            mesh.indices.len()
        ));
    }
    if remove_mesh {
        entity.mesh = None;
        *is_dirty = true;
    }
}

/// 3B2. Material / Texture Component
pub fn draw_texture(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    let mut remove_texture = false;
    if let Some(tex) = &mut entity.texture {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("🎨 Material / Texture Component");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🗑").on_hover_text("Remove Material").clicked() {
                    remove_texture = true;
                }
            });
        });

        ui.horizontal(|ui| {
            ui.label("Albedo Map Path:");
            ui.text_edit_singleline(&mut tex.path);
        });

        ui.horizontal(|ui| {
            ui.label("Color Tint:");
            if ui.color_edit_button_rgb(&mut tex.color).changed() {
                *is_dirty = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Metallic:");
            ui.add(egui::Slider::new(&mut tex.metallic, 0.0..=1.0));
        });

        ui.horizontal(|ui| {
            ui.label("Roughness:");
            ui.add(egui::Slider::new(&mut tex.roughness, 0.0..=1.0));
        });

        let mut has_metallic_map = tex.metallic_map.is_some();
        if ui
            .checkbox(&mut has_metallic_map, "Use Metallic Map")
            .changed()
        {
            if has_metallic_map {
                tex.metallic_map = Some("".to_string());
            } else {
                tex.metallic_map = None;
            }
        }
        if let Some(map_path) = &mut tex.metallic_map {
            ui.horizontal(|ui| {
                ui.label("  Path:");
                ui.text_edit_singleline(map_path);
            });
        }

        let mut has_roughness_map = tex.roughness_map.is_some();
        if ui
            .checkbox(&mut has_roughness_map, "Use Roughness Map")
            .changed()
        {
            if has_roughness_map {
                tex.roughness_map = Some("".to_string());
            } else {
                tex.roughness_map = None;
            }
        }
        if let Some(map_path) = &mut tex.roughness_map {
            ui.horizontal(|ui| {
                ui.label("  Path:");
                ui.text_edit_singleline(map_path);
            });
        }
    }
    if remove_texture {
        entity.texture = None;
        *is_dirty = true;
    }
}

/// 3C. Light configuration
pub fn draw_light(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    let mut remove_light = false;
    if let Some(light) = &mut entity.light {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("💡 Light Component");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🗑").on_hover_text("Remove Light").clicked() {
                    remove_light = true;
                }
            });
        });

        // Type selection (Spot, Directional, Point)
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
                        // Provide sensible default cones if they were zero
                        if light.inner_cone == 0.0 && light.outer_cone == 0.0 {
                            light.inner_cone = 30.0;
                            light.outer_cone = 45.0;
                        }
                    }
                });
        });

        let mut color_arr = [light.color.x, light.color.y, light.color.z];
        let mut light_changed = false;
        ui.horizontal(|ui| {
            ui.label("Color:");
            if ui.color_edit_button_rgb(&mut color_arr).changed() {
                light_changed = true;
            }
        });
        if light_changed {
            light.color = Vec3::new(color_arr[0], color_arr[1], color_arr[2]);
            *is_dirty = true;
        }

        ui.horizontal(|ui| {
            ui.label("Intensity:");
            if ui
                .add(egui::Slider::new(&mut light.intensity, 0.0..=20.0))
                .changed()
            {
                *is_dirty = true;
            }
        });

        // For spot and point, choose distance
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

        // For spot, choose FOV (Outer Cone) and optionally Inner Cone
        if light.light_type == LightType::Spotlight {
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
    }
    if remove_light {
        entity.light = None;
        *is_dirty = true;
    }
}
