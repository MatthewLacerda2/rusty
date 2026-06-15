use glam::Vec3;

use crate::editor::EditorUi;
use crate::scene::{LightComponent, LightType, MeshComponent, Scene};

/// LEFT PANEL: Scene Hierarchy
pub fn draw(editor: &mut EditorUi, ctx: &egui::Context, scene: &mut Scene) {
    egui::SidePanel::left("Hierarchy Panel")
        .width_range(220.0..=300.0)
        .frame(
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(14, 14, 20))
                .inner_margin(10.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(30, 30, 40))),
        )
        .show(ctx, |ui| {
            ui.heading("📁 Scene Hierarchy");
            ui.separator();
            ui.add_space(5.0);

            // List active entities
            egui::ScrollArea::vertical()
                .max_height(250.0)
                .show(ui, |ui| {
                    let root_ids: Vec<u32> = scene
                        .iter()
                        .filter(|e| e.parent_id.is_none())
                        .map(|e| e.id)
                        .collect();
                    for entity_id in root_ids {
                        draw_entity_node(
                            ui,
                            entity_id,
                            scene,
                            &mut editor.selected_entity_id,
                            &mut editor.selected_asset_path,
                            0.0,
                        );
                    }
                });

            ui.separator();
            ui.add_space(5.0);

            // Add Entity controls
            ui.label("Add Entity:");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut editor.new_entity_name);
            });

            ui.horizontal(|ui| {
                egui::ComboBox::from_label("")
                    .selected_text(&editor.new_entity_type)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut editor.new_entity_type,
                            "Box".to_string(),
                            "Box Mesh",
                        );
                        ui.selectable_value(
                            &mut editor.new_entity_type,
                            "Sphere".to_string(),
                            "Sphere Mesh",
                        );
                        ui.selectable_value(
                            &mut editor.new_entity_type,
                            "Plane".to_string(),
                            "Plane Mesh",
                        );
                        ui.selectable_value(
                            &mut editor.new_entity_type,
                            "Cylinder".to_string(),
                            "Cylinder Mesh",
                        );
                        ui.selectable_value(
                            &mut editor.new_entity_type,
                            "PointLight".to_string(),
                            "💡 Point Light",
                        );
                        ui.selectable_value(
                            &mut editor.new_entity_type,
                            "DirectionalLight".to_string(),
                            "☀️ Dir Light",
                        );
                        ui.selectable_value(
                            &mut editor.new_entity_type,
                            "SpotLight".to_string(),
                            "🔦 Spot Light",
                        );
                    });

                if ui.button("➕ Create").clicked() {
                    create_entity(editor, scene);
                }
            });

            ui.add_space(5.0);
            if let Some(selected_id) = editor.selected_entity_id {
                if ui.button("🗑️ Destroy Selected").clicked() {
                    scene.destroy_entity(selected_id);
                    editor.selected_entity_id = None;
                }
            }
        });
}

fn create_entity(editor: &mut EditorUi, scene: &mut Scene) {
    let new_id = scene.add_entity(editor.new_entity_name.clone());
    let name_lower = editor.new_entity_type.to_lowercase();
    if let Some(mut ent) = scene.get_entity_mut(new_id) {
        if name_lower == "box" {
            let (v, idx) = crate::render::mesh::generate_box(1.0, 1.0, 1.0);
            ent.mesh = Some(MeshComponent {
                primitive_type: "Box".to_string(),
                vertices: v,
                indices: idx,
                is_dirty: crate::scene::DirtyFlag::new(true),
            });
        } else if name_lower == "sphere" {
            let (v, idx) = crate::render::mesh::generate_sphere(1.0, 16, 16);
            ent.mesh = Some(MeshComponent {
                primitive_type: "Sphere".to_string(),
                vertices: v,
                indices: idx,
                is_dirty: crate::scene::DirtyFlag::new(true),
            });
        } else if name_lower == "plane" {
            let (v, idx) = crate::render::mesh::generate_plane(15.0, 15.0);
            ent.mesh = Some(MeshComponent {
                primitive_type: "Plane".to_string(),
                vertices: v,
                indices: idx,
                is_dirty: crate::scene::DirtyFlag::new(true),
            });
        } else if name_lower == "cylinder" {
            let (v, idx) = crate::render::mesh::generate_cylinder(
                Vec3::new(0.0, -0.5, 0.0),
                Vec3::new(0.0, 0.5, 0.0),
                0.5,
                12,
            );
            ent.mesh = Some(MeshComponent {
                primitive_type: "Cylinder".to_string(),
                vertices: v,
                indices: idx,
                is_dirty: crate::scene::DirtyFlag::new(true),
            });
        } else if name_lower == "pointlight" {
            ent.light = Some(LightComponent {
                light_type: LightType::Point,
                color: Vec3::ONE,
                intensity: 1.5,
                range: 10.0,
                inner_cone: 30.0,
                outer_cone: 45.0,
            });
        } else if name_lower == "directionallight" {
            ent.light = Some(LightComponent {
                light_type: LightType::Directional,
                color: Vec3::new(1.0, 0.95, 0.8),
                intensity: 2.0,
                range: 0.0,
                inner_cone: 0.0,
                outer_cone: 0.0,
            });
        } else if name_lower == "spotlight" {
            ent.light = Some(LightComponent {
                light_type: LightType::Spotlight,
                color: Vec3::ONE,
                intensity: 2.0,
                range: 15.0,
                inner_cone: 30.0,
                outer_cone: 45.0,
            });
        }
        editor.selected_entity_id = Some(new_id);
        editor.selected_asset_path = None; // clear asset selection
        editor.is_dirty = true;
    }
}

fn draw_entity_node(
    ui: &mut egui::Ui,
    entity_id: u32,
    scene: &Scene,
    selected_entity_id: &mut Option<u32>,
    selected_asset_path: &mut Option<String>,
    depth: f32,
) {
    let children: Vec<u32> = {
        let entity = match scene.get_entity(entity_id) {
            Some(e) => e,
            None => return,
        };
        let is_selected = *selected_entity_id == Some(entity.id);

        let display_label = if let Some(h) = &entity.health {
            if h.is_dead {
                format!("💀 {} (Dead)", entity.name)
            } else {
                format!("👾 {}", entity.name)
            }
        } else if entity.light.is_some() {
            format!("💡 {}", entity.name)
        } else {
            format!("📦 {}", entity.name)
        };

        ui.horizontal(|ui| {
            ui.add_space(depth * 15.0);

            let prefix = if !entity.children.is_empty() {
                "▼ "
            } else {
                "  "
            };
            let response = ui.selectable_label(is_selected, format!("{}{}", prefix, display_label));
            if response.clicked() {
                if is_selected {
                    *selected_entity_id = None;
                } else {
                    *selected_entity_id = Some(entity.id);
                    *selected_asset_path = None; // Deselect active asset inspection
                }
            }
        });

        entity.children.clone()
    };

    for child_id in children {
        draw_entity_node(
            ui,
            child_id,
            scene,
            selected_entity_id,
            selected_asset_path,
            depth + 1.0,
        );
    }
}
