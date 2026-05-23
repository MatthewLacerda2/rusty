use std::rc::Rc;
use std::cell::RefCell;
use std::fs;
use std::path::Path;
use glam::Vec3;

use crate::core::scene::{Scene, Entity, TransformComponent, MeshComponent, TextureComponent, ScriptComponent, AnimatorComponent, LightComponent, LightType, ColliderComponent, ColliderShape, RigidBodyComponent, HealthComponent};
use crate::scripting::{ConsoleLogs, LogLevel};
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;

pub struct EditorUi {
    pub selected_entity_id: Option<u32>,
    pub is_dirty: bool,
    new_entity_name: String,
    new_entity_type: String,
    new_script_path: String,
    assets_scripts: Vec<String>,
    assets_textures: Vec<String>,
}

impl EditorUi {
    pub fn new() -> Self {
        Self {
            selected_entity_id: None,
            is_dirty: true,
            new_entity_name: "New Primitive".to_string(),
            new_entity_type: "Box".to_string(),
            new_script_path: "project/assets/scripts/bot.lua".to_string(),
            assets_scripts: Vec::new(),
            assets_textures: Vec::new(),
        }
    }

    /// Set up futuristic sci-fi dark theme styling in egui
    pub fn apply_theme(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        
        let visuals = &mut style.visuals;
        visuals.dark_mode = true;
        visuals.override_text_color = Some(egui::Color32::from_rgb(220, 225, 240));
        
        // Deep space background fills
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(10, 10, 15);
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(32, 28, 48));
        
        // Slate violet buttons in inactive state
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(20, 18, 30);
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 50, 95));
        
        // Glowing cyan highlights for hover states
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(28, 25, 45);
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(0, 229, 255));
        
        // Glowing neon aqua for active click states
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(35, 30, 60);
        visuals.widgets.active.fg_stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(0, 242, 254));
        
        visuals.selection.bg_fill = egui::Color32::from_rgb(48, 40, 90);
        
        visuals.window_rounding = egui::Rounding::same(10.0);
        visuals.widgets.inactive.rounding = egui::Rounding::same(5.0);
        visuals.widgets.hovered.rounding = egui::Rounding::same(5.0);
        visuals.widgets.active.rounding = egui::Rounding::same(5.0);
        
        ctx.set_style(style);
    }

    /// Read local assets folders to populate Asset Browser
    pub fn scan_assets(&mut self) {
        self.assets_scripts.clear();
        self.assets_textures.clear();

        // Scan scripts
        if let Ok(entries) = fs::read_dir("project/assets/scripts") {
            for entry in entries.flatten() {
                if let Some(path_str) = entry.path().to_str() {
                    if path_str.ends_with(".lua") {
                        self.assets_scripts.push(path_str.replace("\\", "/"));
                    }
                }
            }
        }

        // Scan textures
        if let Ok(entries) = fs::read_dir("project/assets/textures") {
            for entry in entries.flatten() {
                if let Some(path_str) = entry.path().to_str() {
                    let path_lower = path_str.to_lowercase();
                    if path_lower.ends_with(".png") || path_lower.ends_with(".tga") {
                        self.assets_textures.push(path_str.replace("\\", "/"));
                    }
                }
            }
        }
    }

    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        scene: &mut Scene,
        console: &mut ConsoleLogs,
        nav: &mut NavigationGraph,
        is_playing: &mut bool,
        fps: f32,
        frame_time: f32,
    ) {
        self.apply_theme(ctx);
        self.scan_assets();

        // Push editor selection to scene (editor UI is the authority)
        scene.selected_entity_id = self.selected_entity_id;

        // 1. TOP HEADER PANEL (Controls engine state) — ALWAYS VISIBLE
        egui::TopBottomPanel::top("Header Panel").frame(
            egui::Frame::none().fill(egui::Color32::from_rgb(14, 14, 22))
                .inner_margin(8.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(26, 24, 38)))
        ).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🛰️ ANTIGRAVITY ENGINE");
                ui.separator();

                // Play / Stop controls with purple highlight on the active state button
                let purple_bg = egui::Color32::from_rgb(90, 50, 180);

                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    // Play button: purple bg when in PlayMode
                    let play_btn = if *is_playing {
                        egui::Button::new(egui::RichText::new("▶ Play").color(egui::Color32::WHITE).strong())
                            .fill(purple_bg)
                    } else {
                        egui::Button::new(egui::RichText::new("▶ Play").color(egui::Color32::GRAY))
                    };
                    if ui.add(play_btn).clicked() {
                        *is_playing = true;
                    }

                    // Stop button: purple bg when in EditorMode
                    let stop_btn = if !*is_playing {
                        egui::Button::new(egui::RichText::new("■ Stop").color(egui::Color32::WHITE).strong())
                            .fill(purple_bg)
                    } else {
                        egui::Button::new(egui::RichText::new("■ Stop").color(egui::Color32::GRAY))
                    };
                    if ui.add(stop_btn).clicked() {
                        *is_playing = false;
                        scene.selected_entity_id = None;
                        self.selected_entity_id = None;
                    }

                    ui.separator();
                    ui.label(format!("Mode: {}", if *is_playing { "🎮 PLAYMODE" } else { "🛠️ EDITORMODE" }));

                    if !*is_playing {
                        ui.separator();
                        if ui.button("💾 Save").clicked() {
                            if let Err(err) = scene.save_to_file("project/scenes/demo.scene") {
                                console.error(format!("Failed to save scene: {}", err));
                            } else {
                                console.info("Scene saved successfully to project/scenes/demo.scene".to_string());
                            }
                        }
                        if ui.button("📂 Load").clicked() {
                            if let Err(err) = scene.load_from_file("project/scenes/demo.scene") {
                                console.error(format!("Failed to load scene: {}", err));
                            } else {
                                console.info("Scene loaded successfully from project/scenes/demo.scene".to_string());
                                self.selected_entity_id = None;
                                scene.selected_entity_id = None;
                                nav.bake(scene);
                            }
                        }
                    }

                    // Statistics alignment on right side
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("Frame Time: {:.2} ms", frame_time));
                        ui.separator();
                        ui.label(format!("FPS: {:.0}", fps));
                    });
                });
            });
        });

        // If in PlayMode, hide side editor bars to maximize immersive viewport renders
        if *is_playing {
            return;
        }

        // 2. LEFT PANEL: Scene Hierarchy
        egui::SidePanel::left("Hierarchy Panel")
            .width_range(220.0..=300.0)
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(10, 10, 15)).inner_margin(10.0))
            .show(ctx, |ui| {
                ui.heading("📁 Scene Hierarchy");
                ui.separator();
                ui.add_space(5.0);

                // List active entities
                egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                    for entity in &scene.entities {
                        if entity.parent_id.is_none() {
                            draw_entity_node(ui, entity.id, scene, &mut self.selected_entity_id, 0.0);
                        }
                    }
                });

                ui.separator();
                ui.add_space(5.0);

                // Add Entity controls
                ui.label("Add Entity:");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.new_entity_name);
                });
                
                ui.horizontal(|ui| {
                    egui::ComboBox::from_label("")
                        .selected_text(&self.new_entity_type)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.new_entity_type, "Box".to_string(), "Box Mesh");
                            ui.selectable_value(&mut self.new_entity_type, "Sphere".to_string(), "Sphere Mesh");
                            ui.selectable_value(&mut self.new_entity_type, "Plane".to_string(), "Plane Mesh");
                            ui.selectable_value(&mut self.new_entity_type, "Cylinder".to_string(), "Cylinder Mesh");
                            ui.selectable_value(&mut self.new_entity_type, "Point Light".to_string(), "Point Light");
                            ui.selectable_value(&mut self.new_entity_type, "Spotlight".to_string(), "Spotlight");
                        });

                    if ui.button("➕ Add").clicked() {
                        let name = self.new_entity_name.clone();
                        let id = scene.add_entity(name.clone());
                        let ent = scene.get_entity_mut(id).unwrap();

                        // Configure mesh type
                        match self.new_entity_type.as_str() {
                            "Box" => {
                                let (v, idx) = crate::primitives::generate_box(1.5, 1.5, 1.5);
                                ent.mesh = Some(MeshComponent { primitive_type: "Box".to_string(), vertices: v, indices: idx, is_dirty: std::cell::Cell::new(true) });
                                ent.collider = Some(ColliderComponent {
                                    active: true,
                                    shape: ColliderShape::Box { size: Vec3::new(1.5, 1.5, 1.5) },
                                    is_trigger: false,
                                    aabb_min: Vec3::ZERO,
                                    aabb_max: Vec3::ZERO,
                                });
                            }
                            "Sphere" => {
                                let (v, idx) = crate::primitives::generate_sphere(1.0, 16, 16);
                                ent.mesh = Some(MeshComponent { primitive_type: "Sphere".to_string(), vertices: v, indices: idx, is_dirty: std::cell::Cell::new(true) });
                                ent.collider = Some(ColliderComponent {
                                    active: true,
                                    shape: ColliderShape::Sphere { radius: 1.0 },
                                    is_trigger: false,
                                    aabb_min: Vec3::ZERO,
                                    aabb_max: Vec3::ZERO,
                                });
                            }
                            "Plane" => {
                                let (v, idx) = crate::primitives::generate_plane(15.0, 15.0);
                                ent.mesh = Some(MeshComponent { primitive_type: "Plane".to_string(), vertices: v, indices: idx, is_dirty: std::cell::Cell::new(true) });
                                ent.collider = Some(ColliderComponent {
                                    active: true,
                                    shape: ColliderShape::Box { size: Vec3::new(15.0, 0.1, 15.0) },
                                    is_trigger: false,
                                    aabb_min: Vec3::ZERO,
                                    aabb_max: Vec3::ZERO,
                                });
                            }
                            "Cylinder" => {
                                let (v, idx) = crate::primitives::generate_cylinder(Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 0.7, 16);
                                ent.mesh = Some(MeshComponent { primitive_type: "Cylinder".to_string(), vertices: v, indices: idx, is_dirty: std::cell::Cell::new(true) });
                                ent.collider = Some(ColliderComponent {
                                    active: true,
                                    shape: ColliderShape::Cylinder { radius: 0.7, height: 2.0 },
                                    is_trigger: false,
                                    aabb_min: Vec3::ZERO,
                                    aabb_max: Vec3::ZERO,
                                });
                            }
                            "Point Light" => {
                                ent.transform.position = Vec3::new(0.0, 3.0, 0.0);
                                ent.light = Some(LightComponent { light_type: LightType::Point, color: Vec3::new(1.0, 1.0, 1.0), intensity: 2.0, range: 10.0, inner_cone: 0.0, outer_cone: 0.0 });
                                // Light mesh visualizer
                                let (v, idx) = crate::primitives::generate_sphere(0.2, 8, 8);
                                ent.mesh = Some(MeshComponent { primitive_type: "Sphere".to_string(), vertices: v, indices: idx, is_dirty: std::cell::Cell::new(true) });
                            }
                            "Spotlight" => {
                                ent.transform.position = Vec3::new(0.0, 4.0, 0.0);
                                ent.transform.set_euler_angles(Vec3::new(-90.0, 0.0, 0.0)); // Downward
                                ent.light = Some(LightComponent { light_type: LightType::Spotlight, color: Vec3::new(1.0, 1.0, 1.0), intensity: 3.0, range: 15.0, inner_cone: 15.0, outer_cone: 30.0 });
                                // Light mesh visualizer
                                let (v, idx) = crate::primitives::generate_cylinder(Vec3::new(0.0, -0.2, 0.0), Vec3::new(0.0, 0.2, 0.0), 0.15, 8);
                                ent.mesh = Some(MeshComponent { primitive_type: "Cylinder".to_string(), vertices: v, indices: idx, is_dirty: std::cell::Cell::new(true) });
                            }
                            _ => {}
                        }
                        ent.update_collider(None);
                        self.selected_entity_id = Some(id);
                        self.new_entity_name = format!("New Primitive {}", id + 1);
                    }
                });

                ui.add_space(5.0);
                if let Some(selected_id) = self.selected_entity_id {
                    if ui.button("🗑️ Destroy Selected").clicked() {
                        scene.destroy_entity(selected_id);
                        self.selected_entity_id = None;
                    }
                }
            });

        // 3. RIGHT PANEL: Inspector (Mutates properties of the selected Entity)
        egui::SidePanel::right("Inspector Panel")
            .width_range(260.0..=340.0)
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(10, 10, 15)).inner_margin(10.0))
            .show(ctx, |ui| {
                ui.heading("🔬 Properties Inspector");
                ui.separator();
                ui.add_space(5.0);

                if let Some(selected_id) = self.selected_entity_id {
                    let mut pending_parent_change = None;
                    let mut pending_nav_bake = false;

                    let current_parent_id = scene.get_entity(selected_id).and_then(|e| e.parent_id);
                    let parent_mat = current_parent_id.map(|p| scene.compute_world_matrix(p));
                    let selected_parent_name = if let Some(p_id) = current_parent_id {
                        scene.get_entity(p_id).map(|e| e.name.clone()).unwrap_or("None".to_string())
                    } else {
                        "None".to_string()
                    };

                    let valid_parents: Vec<(u32, String)> = scene.entities.iter()
                        .filter(|e| e.id != selected_id)
                        .filter(|e| {
                            let mut curr = e.id;
                            while let Some(ancestor) = scene.get_entity(curr).and_then(|x| x.parent_id) {
                                if ancestor == selected_id {
                                    return false;
                                }
                                curr = ancestor;
                            }
                            true
                        })
                        .map(|e| (e.id, e.name.clone()))
                        .collect();

                    if let Some(entity) = scene.get_entity_mut(selected_id) {
                        // Editable name and status
                        ui.horizontal(|ui| {
                            ui.label("Name:");
                            ui.text_edit_singleline(&mut entity.name);
                        });
                        ui.checkbox(&mut entity.active, "Active / Visible");
                        if ui.checkbox(&mut entity.is_static, "Static (blocks navmesh)").changed() {
                            pending_nav_bake = true;
                        }
                        ui.separator();

                        // 3A. Transform Editor
                        ui.heading("📐 Transform");
                        let trans = &mut entity.transform;

                        let mut pos_changed = false;
                        let mut rot_changed = false;
                        let mut scl_changed = false;

                        ui.label("Position:");
                        ui.horizontal(|ui| {
                            ui.label("X"); pos_changed |= ui.add(egui::DragValue::new(&mut trans.position.x).speed(0.1)).changed();
                            ui.label("Y"); pos_changed |= ui.add(egui::DragValue::new(&mut trans.position.y).speed(0.1)).changed();
                            ui.label("Z"); pos_changed |= ui.add(egui::DragValue::new(&mut trans.position.z).speed(0.1)).changed();
                        });

                        ui.label("Rotation (Degrees):");
                        let mut euler = trans.euler_angles();
                        ui.horizontal(|ui| {
                            ui.label("X"); rot_changed |= ui.add(egui::DragValue::new(&mut euler.x).speed(1.0).clamp_range(-180.0..=180.0)).changed();
                            ui.label("Y"); rot_changed |= ui.add(egui::DragValue::new(&mut euler.y).speed(1.0).clamp_range(-180.0..=180.0)).changed();
                            ui.label("Z"); rot_changed |= ui.add(egui::DragValue::new(&mut euler.z).speed(1.0).clamp_range(-180.0..=180.0)).changed();
                        });
                        if rot_changed {
                            trans.set_euler_angles(euler);
                        }

                        ui.label("Scale:");
                        ui.horizontal(|ui| {
                            ui.label("X"); scl_changed |= ui.add(egui::DragValue::new(&mut trans.scale.x).speed(0.05).clamp_range(0.01..=20.0)).changed();
                            ui.label("Y"); scl_changed |= ui.add(egui::DragValue::new(&mut trans.scale.y).speed(0.05).clamp_range(0.01..=20.0)).changed();
                            ui.label("Z"); scl_changed |= ui.add(egui::DragValue::new(&mut trans.scale.z).speed(0.05).clamp_range(0.01..=20.0)).changed();
                        });

                        if pos_changed || rot_changed || scl_changed {
                            entity.update_collider(parent_mat);
                            self.is_dirty = true;
                            if entity.is_static {
                                pending_nav_bake = true;
                            }
                        }

                        // Parenting Editor
                        ui.separator();
                        ui.heading("🔗 Parent-Child Link");
                        
                        let mut current_sel = selected_parent_name.clone();
                        egui::ComboBox::from_label("Parent")
                            .selected_text(selected_parent_name.clone())
                            .show_ui(ui, |ui| {
                                if ui.selectable_value(&mut current_sel, "None".to_string(), "None").clicked() {
                                    pending_parent_change = Some(None);
                                }
                                for (candidate_id, name) in &valid_parents {
                                    if ui.selectable_value(&mut current_sel, name.clone(), name).clicked() {
                                        pending_parent_change = Some(Some(*candidate_id));
                                    }
                                }
                            });

                        // 3B. Mesh details
                        if let Some(mesh) = &entity.mesh {
                            ui.separator();
                            ui.heading("📦 Mesh Filter");
                            ui.label(format!("Type: {} primitive", mesh.primitive_type));
                            ui.label(format!("Geometry: {} verts | {} indices", mesh.vertices.len(), mesh.indices.len()));
                        }

                        // 3B2. Material / Texture Component
                        if let Some(tex) = &mut entity.texture {
                            ui.separator();
                            ui.heading("🎨 Material / Texture Component");
                            
                            ui.horizontal(|ui| {
                                ui.label("Albedo Map Path:");
                                ui.text_edit_singleline(&mut tex.path);
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
                            if ui.checkbox(&mut has_metallic_map, "Use Metallic Map").changed() {
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
                            if ui.checkbox(&mut has_roughness_map, "Use Roughness Map").changed() {
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
                            
                            if ui.button("🗑 Remove Material").clicked() {
                                entity.texture = None;
                            }
                        }

                        // 3C. Light configuration
                        if let Some(light) = &mut entity.light {
                            ui.separator();
                            ui.heading("💡 Light Component");
                            
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
                                self.is_dirty = true;
                            }

                            ui.horizontal(|ui| {
                                ui.label("Intensity:");
                                if ui.add(egui::Slider::new(&mut light.intensity, 0.0..=20.0)).changed() {
                                    self.is_dirty = true;
                                }
                            });

                            if light.light_type == LightType::Point || light.light_type == LightType::Spotlight {
                                ui.horizontal(|ui| {
                                    ui.label("Range:");
                                    if ui.add(egui::Slider::new(&mut light.range, 0.1..=100.0)).changed() {
                                        self.is_dirty = true;
                                    }
                                });
                            }

                            if light.light_type == LightType::Spotlight {
                                ui.horizontal(|ui| {
                                    ui.label("Inner Cone:");
                                    if ui.add(egui::Slider::new(&mut light.inner_cone, 0.0..=90.0)).changed() {
                                        self.is_dirty = true;
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Outer Cone:");
                                    if ui.add(egui::Slider::new(&mut light.outer_cone, 0.0..=90.0)).changed() {
                                        self.is_dirty = true;
                                    }
                                });
                            }
                        }

                        // 3D. Script bindings
                        if let Some(script) = &mut entity.script {
                            ui.separator();
                            ui.heading("📜 Lua Script Component");
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut script.path);
                            });
                            
                            // Check file exists
                            let exists = Path::new(&script.path).exists();
                            if exists {
                                ui.colored_label(egui::Color32::from_rgb(0, 242, 254), "✔ Loaded and ready to run");
                            } else {
                                ui.colored_label(egui::Color32::from_rgb(255, 60, 100), "❌ File not found!");
                            }

                            if ui.button("🗑 Remove Script").clicked() {
                                entity.script = None;
                            }
                        }

                        // 3E. Health component
                        if let Some(health) = &mut entity.health {
                            ui.separator();
                            ui.heading("❤️ Health Component");
                            ui.horizontal(|ui| {
                                ui.label("Current HP:");
                                ui.add(egui::DragValue::new(&mut health.current_health));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Max HP:");
                                ui.add(egui::DragValue::new(&mut health.max_health));
                            });
                            ui.checkbox(&mut health.is_dead, "Is Dead");
                        }

                        // 3EE. Collider Component
                        if let Some(collider) = &mut entity.collider {
                            ui.separator();
                            ui.heading("🟢 Collider Component");
                            ui.checkbox(&mut collider.active, "Active");
                            ui.checkbox(&mut collider.is_trigger, "Is Trigger");

                            let mut shape_type = match &collider.shape {
                                ColliderShape::Box { .. } => "Box",
                                ColliderShape::Sphere { .. } => "Sphere",
                                ColliderShape::Cylinder { .. } => "Cylinder",
                            };

                            let old_shape_type = shape_type;
                            egui::ComboBox::from_label("Shape")
                                .selected_text(shape_type)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut shape_type, "Box", "Box");
                                    ui.selectable_value(&mut shape_type, "Sphere", "Sphere");
                                    ui.selectable_value(&mut shape_type, "Cylinder", "Cylinder");
                                });

                            if shape_type != old_shape_type {
                                collider.shape = match shape_type {
                                    "Box" => ColliderShape::Box { size: Vec3::ONE },
                                    "Sphere" => ColliderShape::Sphere { radius: 0.5 },
                                    "Cylinder" => ColliderShape::Cylinder { radius: 0.5, height: 1.0 },
                                    _ => ColliderShape::Box { size: Vec3::ONE },
                                };
                                pending_nav_bake = true;
                            }

                            match &mut collider.shape {
                                ColliderShape::Box { size } => {
                                    ui.horizontal(|ui| {
                                        ui.label("Size:");
                                        ui.add(egui::DragValue::new(&mut size.x).speed(0.05).clamp_range(0.01..=100.0));
                                        ui.add(egui::DragValue::new(&mut size.y).speed(0.05).clamp_range(0.01..=100.0));
                                        ui.add(egui::DragValue::new(&mut size.z).speed(0.05).clamp_range(0.01..=100.0));
                                    });
                                }
                                ColliderShape::Sphere { radius } => {
                                    ui.horizontal(|ui| {
                                        ui.label("Radius:");
                                        ui.add(egui::DragValue::new(radius).speed(0.05).clamp_range(0.01..=100.0));
                                    });
                                }
                                ColliderShape::Cylinder { radius, height } => {
                                    ui.horizontal(|ui| {
                                        ui.label("Radius:");
                                        ui.add(egui::DragValue::new(radius).speed(0.05).clamp_range(0.01..=100.0));
                                        ui.label("Height:");
                                        ui.add(egui::DragValue::new(height).speed(0.05).clamp_range(0.01..=100.0));
                                    });
                                }
                            }

                            if ui.button("🗑 Remove Collider").clicked() {
                                entity.collider = None;
                            }
                        }

                        // 3EG. RigidBody Component
                        if let Some(rb) = &mut entity.rigidbody {
                            ui.separator();
                            ui.heading("📦 RigidBody Component");
                            ui.checkbox(&mut rb.active, "Active");
                            ui.checkbox(&mut rb.is_kinematic, "Is Kinematic");
                            ui.checkbox(&mut rb.use_gravity, "Use Gravity");

                            ui.horizontal(|ui| {
                                ui.label("Mass:");
                                ui.add(egui::DragValue::new(&mut rb.mass).speed(0.05).clamp_range(0.01..=1000.0));
                            });

                            ui.horizontal(|ui| {
                                ui.label("Velocity:");
                                ui.add(egui::DragValue::new(&mut rb.velocity.x).speed(0.1));
                                ui.add(egui::DragValue::new(&mut rb.velocity.y).speed(0.1));
                                ui.add(egui::DragValue::new(&mut rb.velocity.z).speed(0.1));
                            });

                            if ui.button("🗑 Remove RigidBody").clicked() {
                                entity.rigidbody = None;
                            }
                        }

                        // 3F. Add Component Option
                        ui.separator();
                        ui.menu_button("➕ Add Component", |ui| {
                            if entity.script.is_none() {
                                if ui.button("Script Component").clicked() {
                                    entity.script = Some(ScriptComponent { path: "project/assets/scripts/bot.lua".to_string(), is_loaded: false });
                                    ui.close_menu();
                                }
                            }
                            if entity.light.is_none() {
                                if ui.button("Point Light Component").clicked() {
                                    entity.light = Some(LightComponent { light_type: LightType::Point, color: Vec3::ONE, intensity: 1.5, range: 10.0, inner_cone: 0.0, outer_cone: 0.0 });
                                    ui.close_menu();
                                }
                            }
                            if entity.health.is_none() {
                                if ui.button("Health Component (Enemies)").clicked() {
                                    entity.health = Some(HealthComponent { current_health: 100.0, max_health: 100.0, is_dead: false });
                                    entity.animator = Some(AnimatorComponent { current_clip: "Idle".to_string(), time: 0.0, speed: 2.0, is_playing: true, freeze: false });
                                    ui.close_menu();
                                }
                            }
                            if entity.collider.is_none() {
                                if ui.button("Collider Component").clicked() {
                                    entity.collider = Some(ColliderComponent {
                                        active: true,
                                        shape: ColliderShape::Box { size: Vec3::ONE },
                                        is_trigger: false,
                                        aabb_min: Vec3::ZERO,
                                        aabb_max: Vec3::ZERO,
                                    });
                                    ui.close_menu();
                                }
                            }
                            if entity.rigidbody.is_none() {
                                if ui.button("RigidBody Component").clicked() {
                                    entity.rigidbody = Some(RigidBodyComponent {
                                        active: true,
                                        is_kinematic: false,
                                        mass: 1.0,
                                        velocity: Vec3::ZERO,
                                        use_gravity: true,
                                    });
                                    ui.close_menu();
                                }
                            }
                            if entity.texture.is_none() {
                                if ui.button("Material / Texture Component").clicked() {
                                    entity.texture = Some(TextureComponent {
                                        path: "".to_string(),
                                        is_dirty: true,
                                        metallic: 0.0,
                                        roughness: 0.5,
                                        metallic_map: None,
                                        roughness_map: None,
                                    });
                                    ui.close_menu();
                                }
                            }
                        });
                    }
                    
                    if let Some(new_parent) = pending_parent_change {
                        let _ = scene.set_parent(selected_id, new_parent);
                    }
                    if pending_nav_bake {
                        nav.bake(scene);
                    }
                } else {
                    ui.heading("🌍 Global Scene Settings");
                    ui.separator();
                    ui.add_space(5.0);

                    // 1. Skybox Path
                    ui.horizontal(|ui| {
                        ui.label("Skybox:");
                        let response = ui.text_edit_singleline(&mut scene.skybox_path);
                        if response.changed() {
                            self.is_dirty = true;
                        }
                    });
                    ui.colored_label(egui::Color32::from_rgb(120, 120, 140), "Provide a path to a panoramic image\n(e.g. assets/textures/sky.png)");
                    ui.add_space(8.0);

                    // 2. Ambient Light Color
                    ui.label("Ambient Color:");
                    ui.horizontal(|ui| {
                        let mut color_arr = [scene.ambient_color.x, scene.ambient_color.y, scene.ambient_color.z];
                        if ui.color_edit_button_rgb(&mut color_arr).changed() {
                            scene.ambient_color = Vec3::new(color_arr[0], color_arr[1], color_arr[2]);
                            self.is_dirty = true;
                        }
                    });
                    ui.add_space(8.0);

                    // 3. Ambient Light Intensity
                    ui.label("Ambient Intensity:");
                    let response = ui.add(egui::Slider::new(&mut scene.ambient_intensity, 0.0..=5.0).text("intensity"));
                    if response.changed() {
                        self.is_dirty = true;
                    }
                    
                    ui.add_space(15.0);
                    ui.separator();
                    ui.add_space(5.0);
                    ui.colored_label(egui::Color32::from_rgb(100, 100, 130), "Select an entity from Hierarchy\nto inspect properties.");
                }
            });

        // 4. BOTTOM PANEL: Split into Asset Browser (Left) and Developer Log Console (Right)
        egui::TopBottomPanel::bottom("Bottom Panel")
            .min_height(160.0)
            .frame(
                egui::Frame::none().fill(egui::Color32::from_rgb(10, 10, 15))
                    .inner_margin(8.0)
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(26, 24, 38)))
            )
            .show(ctx, |ui| {
                ui.columns(2, |columns| {
                    // LEFT COLUMN: Asset Browser
                    let ui_l = &mut columns[0];
                    ui_l.heading("📁 Asset Browser");
                    ui_l.separator();
                    ui_l.add_space(3.0);

                    egui::ScrollArea::vertical().id_source("AssetsScroll").max_height(120.0).show(ui_l, |ui| {
                        ui.label("📜 Scripts:");
                        if self.assets_scripts.is_empty() {
                            ui.colored_label(egui::Color32::GRAY, "  (Empty scripts folder)");
                        } else {
                            for path in &self.assets_scripts {
                                let filename = Path::new(path).file_name().and_then(|f| f.to_str()).unwrap_or(path);
                                ui.horizontal(|ui| {
                                    ui.label(format!("  📄 {}", filename));
                                    if let Some(selected_id) = self.selected_entity_id {
                                        if ui.small_button("Attach to Selected").clicked() {
                                            if let Some(ent) = scene.get_entity_mut(selected_id) {
                                                ent.script = Some(ScriptComponent { path: path.clone(), is_loaded: false });
                                            }
                                        }
                                    }
                                });
                            }
                        }

                        ui.add_space(5.0);
                        ui.label("🖼️ Textures:");
                        if self.assets_textures.is_empty() {
                            ui.colored_label(egui::Color32::GRAY, "  (Empty textures folder)");
                        } else {
                            for path in &self.assets_textures {
                                let filename = Path::new(path).file_name().and_then(|f| f.to_str()).unwrap_or(path);
                                ui.horizontal(|ui| {
                                    ui.label(format!("  🖼️ {}", filename));
                                    if let Some(selected_id) = self.selected_entity_id {
                                        if ui.small_button("Apply to Selected").clicked() {
                                            if let Some(ent) = scene.get_entity_mut(selected_id) {
                                                ent.texture = Some(TextureComponent {
                                                    path: path.clone(),
                                                    is_dirty: true,
                                                    metallic: 0.0,
                                                    roughness: 0.5,
                                                    metallic_map: None,
                                                    roughness_map: None,
                                                });
                                            }
                                        }
                                    }
                                });
                            }
                        }
                    });

                    // RIGHT COLUMN: Developer Console
                    let ui_r = &mut columns[1];
                    ui_r.horizontal(|ui| {
                        ui.heading("📟 Developer Console");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Clear").clicked() {
                                console.messages.clear();
                            }
                        });
                    });
                    ui_r.separator();
                    ui_r.add_space(3.0);

                    egui::ScrollArea::vertical().id_source("ConsoleScroll").max_height(120.0).show(ui_r, |ui| {
                        if console.messages.is_empty() {
                            ui.colored_label(egui::Color32::from_rgb(100, 100, 130), "  No execution logs yet. Logs will print when running.");
                        } else {
                            for (msg, level) in &console.messages {
                                let color = match level {
                                    LogLevel::Info => egui::Color32::from_rgb(220, 220, 230),      // White
                                    LogLevel::Warning => egui::Color32::from_rgb(255, 200, 50),     // Yellow
                                    LogLevel::Error => egui::Color32::from_rgb(255, 60, 100),       // Bright Red
                                };
                                ui.colored_label(color, format!("  {}", msg));
                            }
                        }
                    });
                });
            });
    }
}

fn draw_entity_node(
    ui: &mut egui::Ui,
    entity_id: u32,
    scene: &Scene,
    selected_entity_id: &mut Option<u32>,
    depth: f32,
) {
    if let Some(entity) = scene.get_entity(entity_id) {
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
            ui.add_space(depth * 15.0); // Indentation for nesting
            
            // Parent-child expansion prefix
            let prefix = if !entity.children.is_empty() { "▼ " } else { "  " };
            let response = ui.selectable_label(is_selected, format!("{}{}", prefix, display_label));
            if response.clicked() {
                if is_selected {
                    *selected_entity_id = None;
                } else {
                    *selected_entity_id = Some(entity.id);
                }
            }
        });

        // Draw descendants with increased indentation
        for &child_id in &entity.children {
            draw_entity_node(ui, child_id, scene, selected_entity_id, depth + 1.0);
        }
    }
}
