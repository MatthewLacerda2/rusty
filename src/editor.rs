use glam::Vec3;
use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;

pub mod inspectors;

use crate::core::input::InputState;
use crate::core::scene::{
    AnimatorComponent, CameraComponent, ColliderComponent, ColliderShape, Entity, HealthComponent,
    LightComponent, LightType, MeshComponent, NavMeshAgentComponent, RigidBodyComponent, Scene,
    ScriptComponent, TextureComponent, TransformComponent, VisualCorrectionComponent,
};
use crate::navigation::NavigationGraph;
use crate::scripting::{ConsoleLogs, LogLevel};

pub struct EditorUi {
    pub selected_entity_id: Option<u32>,
    pub selected_asset_path: Option<String>,
    pub current_dir: String,
    pub is_dirty: bool,
    new_entity_name: String,
    new_entity_type: String,
    new_script_path: String,
    assets_scripts: Vec<String>,
    assets_textures: Vec<String>,

    // Custom asset inspector properties
    pub asset_image_wrap: String,
    pub asset_image_filter: String,
    pub asset_image_mipmaps: bool,
    pub asset_audio_volume: f32,
    pub asset_audio_pitch: f32,
    pub asset_audio_loop: bool,
    pub asset_audio_playing: bool,
    pub asset_audio_play_time: Option<std::time::Instant>,
    pub asset_model_scale: f32,
    pub asset_model_import_normals: bool,
    pub asset_script_content: String,
    pub active_bottom_tab: String,
}

impl EditorUi {
    pub fn new() -> Self {
        Self {
            selected_entity_id: None,
            selected_asset_path: None,
            current_dir: "project".to_string(),
            is_dirty: true,
            new_entity_name: "New Primitive".to_string(),
            new_entity_type: "Box".to_string(),
            new_script_path: "project/assets/scripts/bot.lua".to_string(),
            assets_scripts: Vec::new(),
            assets_textures: Vec::new(),

            asset_image_wrap: "Repeat".to_string(),
            asset_image_filter: "Linear".to_string(),
            asset_image_mipmaps: true,
            asset_audio_volume: 0.8,
            asset_audio_pitch: 1.0,
            asset_audio_loop: false,
            asset_audio_playing: false,
            asset_audio_play_time: None,
            asset_model_scale: 1.0,
            asset_model_import_normals: true,
            asset_script_content: String::new(),
            active_bottom_tab: "assets".to_string(),
        }
    }

    /// Set up high-end professional dark theme styling with subtle cyan/teal accents (Unreal/Blender style)
    pub fn apply_theme(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();

        let visuals = &mut style.visuals;
        visuals.dark_mode = true;
        visuals.override_text_color = Some(egui::Color32::from_rgb(228, 230, 235));

        // Professional slate-gray-blue obsidian theme
        let bg_dark = egui::Color32::from_rgb(16, 16, 22); // Main panels
        let border_color = egui::Color32::from_rgb(35, 35, 45); // Borders
        let widget_inactive = egui::Color32::from_rgb(28, 28, 36);
        let widget_hovered = egui::Color32::from_rgb(38, 38, 50);
        let widget_active = egui::Color32::from_rgb(46, 46, 62);

        let accent_cyan = egui::Color32::from_rgb(0, 229, 255);
        let accent_teal = egui::Color32::from_rgb(0, 242, 254);

        visuals.widgets.noninteractive.bg_fill = bg_dark;
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, border_color);

        // Inactive widgets
        visuals.widgets.inactive.bg_fill = widget_inactive;
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, border_color);
        visuals.widgets.inactive.rounding = egui::Rounding::same(4.0);

        // Hovered widgets
        visuals.widgets.hovered.bg_fill = widget_hovered;
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.2, accent_cyan);
        visuals.widgets.hovered.rounding = egui::Rounding::same(4.0);

        // Active/Focused widgets
        visuals.widgets.active.bg_fill = widget_active;
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.5, accent_teal);
        visuals.widgets.active.rounding = egui::Rounding::same(4.0);

        visuals.selection.bg_fill = egui::Color32::from_rgb(0, 75, 90);

        visuals.window_rounding = egui::Rounding::same(6.0);

        // Tight spacing and clean layout parameters
        style.spacing.item_spacing = egui::vec2(6.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);

        ctx.set_style(style);
    }

    /// Read local assets folders to populate Asset Browser (legacy scan, kept for compatibility)
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
        _fps: f32,
        _frame_time: f32,
    ) {
        self.apply_theme(ctx);
        self.scan_assets();

        // Push editor selection to scene (editor UI is the authority)
        scene.selected_entity_id = self.selected_entity_id;

        // 1. TOP HEADER PANEL (Controls engine state) — ALWAYS VISIBLE
        egui::TopBottomPanel::top("Header Panel")
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(22, 22, 30))
                    .inner_margin(8.0)
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(30, 30, 40))),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let purple_bg = egui::Color32::from_rgb(124, 77, 255); // Premium indigo

                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let play_btn = if *is_playing {
                            egui::Button::new(
                                egui::RichText::new("▶ Play")
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(purple_bg)
                        } else {
                            egui::Button::new(
                                egui::RichText::new("▶ Play").color(egui::Color32::GRAY),
                            )
                        };
                        if ui.add(play_btn).clicked() {
                            *is_playing = true;
                        }

                        let stop_btn = if !*is_playing {
                            egui::Button::new(
                                egui::RichText::new("■ Stop")
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(purple_bg)
                        } else {
                            egui::Button::new(
                                egui::RichText::new("■ Stop").color(egui::Color32::GRAY),
                            )
                        };
                        if ui.add(stop_btn).clicked() {
                            *is_playing = false;
                            scene.selected_entity_id = None;
                            self.selected_entity_id = None;
                        }

                        ui.separator();
                        ui.label(format!(
                            "Mode: {}",
                            if *is_playing {
                                "🎮 PLAYMODE"
                            } else {
                                "🛠️ EDITORMODE"
                            }
                        ));

                        // Quick scene save/load buttons in header
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("💾 Save Scene").clicked() {
                                if let Err(err) = scene.save_to_file("project/scenes/demo.scene") {
                                    console.error(format!("Failed to save scene: {}", err));
                                } else {
                                    console.info(
                                        "Scene saved successfully to project/scenes/demo.scene"
                                            .to_string(),
                                    );
                                }
                            }
                            if ui.button("📂 Load Scene").clicked() {
                                if let Err(err) = scene.load_from_file("project/scenes/demo.scene")
                                {
                                    console.error(format!("Failed to load scene: {}", err));
                                } else {
                                    console.info(
                                        "Scene loaded successfully from project/scenes/demo.scene"
                                            .to_string(),
                                    );
                                }
                            }
                        });
                    });
                });
            });

        if *is_playing {
            return;
        }

        // 2. LEFT PANEL: Scene Hierarchy
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
                        for entity in &scene.entities {
                            if entity.parent_id.is_none() {
                                draw_entity_node(
                                    ui,
                                    entity.id,
                                    scene,
                                    &mut self.selected_entity_id,
                                    &mut self.selected_asset_path,
                                    0.0,
                                );
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
                            ui.selectable_value(
                                &mut self.new_entity_type,
                                "Box".to_string(),
                                "Box Mesh",
                            );
                            ui.selectable_value(
                                &mut self.new_entity_type,
                                "Sphere".to_string(),
                                "Sphere Mesh",
                            );
                            ui.selectable_value(
                                &mut self.new_entity_type,
                                "Plane".to_string(),
                                "Plane Mesh",
                            );
                            ui.selectable_value(
                                &mut self.new_entity_type,
                                "Cylinder".to_string(),
                                "Cylinder Mesh",
                            );
                            ui.selectable_value(
                                &mut self.new_entity_type,
                                "PointLight".to_string(),
                                "💡 Point Light",
                            );
                            ui.selectable_value(
                                &mut self.new_entity_type,
                                "DirectionalLight".to_string(),
                                "☀️ Dir Light",
                            );
                            ui.selectable_value(
                                &mut self.new_entity_type,
                                "SpotLight".to_string(),
                                "🔦 Spot Light",
                            );
                        });

                    if ui.button("➕ Create").clicked() {
                        let new_id = scene.add_entity(self.new_entity_name.clone());
                        let name_lower = self.new_entity_type.to_lowercase();
                        if let Some(ent) = scene.get_entity_mut(new_id) {
                            if name_lower == "box" {
                                let (v, idx) = crate::render::mesh::generate_box(1.0, 1.0, 1.0);
                                ent.mesh = Some(MeshComponent {
                                    primitive_type: "Box".to_string(),
                                    vertices: v,
                                    indices: idx,
                                    is_dirty: std::cell::Cell::new(true),
                                });
                            } else if name_lower == "sphere" {
                                let (v, idx) = crate::render::mesh::generate_sphere(1.0, 16, 16);
                                ent.mesh = Some(MeshComponent {
                                    primitive_type: "Sphere".to_string(),
                                    vertices: v,
                                    indices: idx,
                                    is_dirty: std::cell::Cell::new(true),
                                });
                            } else if name_lower == "plane" {
                                let (v, idx) = crate::render::mesh::generate_plane(15.0, 15.0);
                                ent.mesh = Some(MeshComponent {
                                    primitive_type: "Plane".to_string(),
                                    vertices: v,
                                    indices: idx,
                                    is_dirty: std::cell::Cell::new(true),
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
                                    is_dirty: std::cell::Cell::new(true),
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
                            self.selected_entity_id = Some(new_id);
                            self.selected_asset_path = None; // clear asset selection
                            self.is_dirty = true;
                        }
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

        // 3. RIGHT PANEL: Properties Inspector
        egui::SidePanel::right("Inspector Panel")
            .width_range(260.0..=340.0)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(14, 14, 20))
                    .inner_margin(10.0)
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(30, 30, 40))),
            )
            .show(ctx, |ui| {
                ui.heading("🔬 Properties Inspector");
                ui.separator();
                ui.add_space(5.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    if let Some(selected_id) = self.selected_entity_id {
                        let mut pending_parent_change = None;
                        let mut pending_nav_bake = false;

                        let current_parent_id =
                            scene.get_entity(selected_id).and_then(|e| e.parent_id);
                        let parent_mat = current_parent_id.map(|p| scene.compute_world_matrix(p));
                        let selected_parent_name = if let Some(p_id) = current_parent_id {
                            scene
                                .get_entity(p_id)
                                .map(|e| e.name.clone())
                                .unwrap_or("None".to_string())
                        } else {
                            "None".to_string()
                        };

                        let valid_parents: Vec<(u32, String)> = scene
                            .entities
                            .iter()
                            .filter(|e| e.id != selected_id)
                            .filter(|e| {
                                let mut curr = e.id;
                                while let Some(ancestor) =
                                    scene.get_entity(curr).and_then(|x| x.parent_id)
                                {
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
                            if entity.camera.is_none() && entity.visual_correction.is_some() {
                                entity.visual_correction = None;
                            }

                            ui.horizontal(|ui| {
                                ui.label("Name:");
                                ui.text_edit_singleline(&mut entity.name);
                            });
                            ui.checkbox(&mut entity.active, "Active / Visible");
                            if ui
                                .checkbox(&mut entity.is_static, "Static (blocks navmesh)")
                                .changed()
                            {
                                pending_nav_bake = true;
                            }
                            ui.separator();

                            // 3A. Transform Editor with Integrated Parenting
                            ui.heading("📐 Transform");
                            let trans = &mut entity.transform;

                            let mut pos_changed = false;
                            let mut rot_changed = false;
                            let mut scl_changed = false;

                            ui.label("Position:");
                            ui.horizontal(|ui| {
                                ui.label("X");
                                pos_changed |= ui
                                    .add(egui::DragValue::new(&mut trans.position.x).speed(0.1))
                                    .changed();
                                ui.label("Y");
                                pos_changed |= ui
                                    .add(egui::DragValue::new(&mut trans.position.y).speed(0.1))
                                    .changed();
                                ui.label("Z");
                                pos_changed |= ui
                                    .add(egui::DragValue::new(&mut trans.position.z).speed(0.1))
                                    .changed();
                            });

                            ui.label("Rotation (Degrees):");
                            let mut euler = trans.euler_angles();
                            ui.horizontal(|ui| {
                                ui.label("X");
                                rot_changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut euler.x)
                                            .speed(1.0)
                                            .clamp_range(-180.0..=180.0),
                                    )
                                    .changed();
                                ui.label("Y");
                                rot_changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut euler.y)
                                            .speed(1.0)
                                            .clamp_range(-180.0..=180.0),
                                    )
                                    .changed();
                                ui.label("Z");
                                rot_changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut euler.z)
                                            .speed(1.0)
                                            .clamp_range(-180.0..=180.0),
                                    )
                                    .changed();
                            });
                            if rot_changed {
                                trans.set_euler_angles(euler);
                            }

                            ui.label("Scale:");
                            ui.horizontal(|ui| {
                                ui.label("X");
                                scl_changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut trans.scale.x)
                                            .speed(0.05)
                                            .clamp_range(0.01..=20.0),
                                    )
                                    .changed();
                                ui.label("Y");
                                scl_changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut trans.scale.y)
                                            .speed(0.05)
                                            .clamp_range(0.01..=20.0),
                                    )
                                    .changed();
                                ui.label("Z");
                                scl_changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut trans.scale.z)
                                            .speed(0.05)
                                            .clamp_range(0.01..=20.0),
                                    )
                                    .changed();
                            });

                            if pos_changed || rot_changed || scl_changed {
                                entity.update_collider(parent_mat);
                                self.is_dirty = true;
                                if entity.is_static {
                                    pending_nav_bake = true;
                                }
                            }

                            // Integrated Parenting directly under Transform
                            ui.add_space(5.0);
                            ui.horizontal(|ui| {
                                ui.label("Parent:");
                                let mut current_sel = selected_parent_name.clone();
                                egui::ComboBox::from_id_source("ParentSelectionCombo")
                                    .selected_text(selected_parent_name.clone())
                                    .show_ui(ui, |ui| {
                                        if ui
                                            .selectable_value(
                                                &mut current_sel,
                                                "None".to_string(),
                                                "None",
                                            )
                                            .clicked()
                                        {
                                            pending_parent_change = Some(None);
                                        }
                                        for (candidate_id, name) in &valid_parents {
                                            if ui
                                                .selectable_value(
                                                    &mut current_sel,
                                                    name.clone(),
                                                    name,
                                                )
                                                .clicked()
                                            {
                                                pending_parent_change = Some(Some(*candidate_id));
                                            }
                                        }
                                    });
                            });

                            // 3B. Mesh details
                            let mut remove_mesh = false;
                            if let Some(mesh) = &entity.mesh {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.heading("📦 Mesh Filter");
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .button("🗑")
                                                .on_hover_text("Remove Mesh Filter")
                                                .clicked()
                                            {
                                                remove_mesh = true;
                                            }
                                        },
                                    );
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
                                self.is_dirty = true;
                            }

                            // 3B2. Material / Texture Component
                            let mut remove_texture = false;
                            if let Some(tex) = &mut entity.texture {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.heading("🎨 Material / Texture Component");
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .button("🗑")
                                                .on_hover_text("Remove Material")
                                                .clicked()
                                            {
                                                remove_texture = true;
                                            }
                                        },
                                    );
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Albedo Map Path:");
                                    ui.text_edit_singleline(&mut tex.path);
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Color Tint:");
                                    if ui.color_edit_button_rgb(&mut tex.color).changed() {
                                        self.is_dirty = true;
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
                                self.is_dirty = true;
                            }

                            // 3C. Light configuration
                            let mut remove_light = false;
                            if let Some(light) = &mut entity.light {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.heading("💡 Light Component");
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .button("🗑")
                                                .on_hover_text("Remove Light")
                                                .clicked()
                                            {
                                                remove_light = true;
                                            }
                                        },
                                    );
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
                                                .selectable_label(
                                                    light.light_type == LightType::Point,
                                                    "Point",
                                                )
                                                .clicked()
                                            {
                                                light.light_type = LightType::Point;
                                                self.is_dirty = true;
                                            }
                                            if ui
                                                .selectable_label(
                                                    light.light_type == LightType::Directional,
                                                    "Directional",
                                                )
                                                .clicked()
                                            {
                                                light.light_type = LightType::Directional;
                                                self.is_dirty = true;
                                            }
                                            if ui
                                                .selectable_label(
                                                    light.light_type == LightType::Spotlight,
                                                    "Spot",
                                                )
                                                .clicked()
                                            {
                                                light.light_type = LightType::Spotlight;
                                                self.is_dirty = true;
                                                // Provide sensible default cones if they were zero
                                                if light.inner_cone == 0.0
                                                    && light.outer_cone == 0.0
                                                {
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
                                    light.color =
                                        Vec3::new(color_arr[0], color_arr[1], color_arr[2]);
                                    self.is_dirty = true;
                                }

                                ui.horizontal(|ui| {
                                    ui.label("Intensity:");
                                    if ui
                                        .add(egui::Slider::new(&mut light.intensity, 0.0..=20.0))
                                        .changed()
                                    {
                                        self.is_dirty = true;
                                    }
                                });

                                // For spot and point, choose distance
                                if light.light_type == LightType::Point
                                    || light.light_type == LightType::Spotlight
                                {
                                    ui.horizontal(|ui| {
                                        ui.label("Distance:");
                                        if ui
                                            .add(egui::Slider::new(&mut light.range, 0.1..=100.0))
                                            .changed()
                                        {
                                            self.is_dirty = true;
                                        }
                                    });
                                }

                                // For spot, choose FOV (Outer Cone) and optionally Inner Cone
                                if light.light_type == LightType::Spotlight {
                                    ui.horizontal(|ui| {
                                        ui.label("FOV (Outer Cone):");
                                        if ui
                                            .add(
                                                egui::Slider::new(
                                                    &mut light.outer_cone,
                                                    0.1..=90.0,
                                                )
                                                .suffix("°"),
                                            )
                                            .changed()
                                        {
                                            self.is_dirty = true;
                                            if light.inner_cone > light.outer_cone {
                                                light.inner_cone = light.outer_cone;
                                            }
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("Inner Cone:");
                                        if ui
                                            .add(
                                                egui::Slider::new(
                                                    &mut light.inner_cone,
                                                    0.0..=90.0,
                                                )
                                                .suffix("°"),
                                            )
                                            .changed()
                                        {
                                            self.is_dirty = true;
                                            if light.inner_cone > light.outer_cone {
                                                light.outer_cone = light.inner_cone;
                                            }
                                        }
                                    });
                                }
                            }
                            if remove_light {
                                entity.light = None;
                                self.is_dirty = true;
                            }

                            // 3D. Script bindings
                            let mut remove_script = false;
                            if let Some(script) = &mut entity.script {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.heading("📜 Lua Script Component");
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .button("🗑")
                                                .on_hover_text("Remove Script")
                                                .clicked()
                                            {
                                                remove_script = true;
                                            }
                                        },
                                    );
                                });
                                ui.horizontal(|ui| {
                                    ui.text_edit_singleline(&mut script.path);
                                });

                                let exists = Path::new(&script.path).exists();
                                if exists {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(0, 242, 254),
                                        "✔ Loaded and ready to run",
                                    );
                                } else {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(255, 60, 100),
                                        "❌ File not found!",
                                    );
                                }
                            }
                            if remove_script {
                                entity.script = None;
                                self.is_dirty = true;
                            }

                            // 3E. Health component
                            let mut remove_health = false;
                            if let Some(health) = &mut entity.health {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.heading("❤️ Health Component");
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .button("🗑")
                                                .on_hover_text("Remove Health")
                                                .clicked()
                                            {
                                                remove_health = true;
                                            }
                                        },
                                    );
                                });
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
                            if remove_health {
                                entity.health = None;
                                entity.animator = None;
                                self.is_dirty = true;
                            }

                            // 3EE. Collider Component
                            let mut remove_collider = false;
                            if let Some(collider) = &mut entity.collider {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.heading("🟢 Collider Component");
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .button("🗑")
                                                .on_hover_text("Remove Collider")
                                                .clicked()
                                            {
                                                remove_collider = true;
                                            }
                                        },
                                    );
                                });
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
                                        ui.selectable_value(
                                            &mut shape_type,
                                            "Cylinder",
                                            "Cylinder",
                                        );
                                    });

                                if shape_type != old_shape_type {
                                    collider.shape = match shape_type {
                                        "Box" => ColliderShape::Box { size: Vec3::ONE },
                                        "Sphere" => ColliderShape::Sphere { radius: 0.5 },
                                        "Cylinder" => ColliderShape::Cylinder {
                                            radius: 0.5,
                                            height: 1.0,
                                        },
                                        _ => ColliderShape::Box { size: Vec3::ONE },
                                    };
                                    pending_nav_bake = true;
                                }

                                match &mut collider.shape {
                                    ColliderShape::Box { size } => {
                                        ui.horizontal(|ui| {
                                            ui.label("Size:");
                                            ui.add(
                                                egui::DragValue::new(&mut size.x)
                                                    .speed(0.05)
                                                    .clamp_range(0.01..=100.0),
                                            );
                                            ui.add(
                                                egui::DragValue::new(&mut size.y)
                                                    .speed(0.05)
                                                    .clamp_range(0.01..=100.0),
                                            );
                                            ui.add(
                                                egui::DragValue::new(&mut size.z)
                                                    .speed(0.05)
                                                    .clamp_range(0.01..=100.0),
                                            );
                                        });
                                    }
                                    ColliderShape::Sphere { radius } => {
                                        ui.horizontal(|ui| {
                                            ui.label("Radius:");
                                            ui.add(
                                                egui::DragValue::new(radius)
                                                    .speed(0.05)
                                                    .clamp_range(0.01..=100.0),
                                            );
                                        });
                                    }
                                    ColliderShape::Cylinder { radius, height } => {
                                        ui.horizontal(|ui| {
                                            ui.label("Radius:");
                                            ui.add(
                                                egui::DragValue::new(radius)
                                                    .speed(0.05)
                                                    .clamp_range(0.01..=100.0),
                                            );
                                            ui.label("Height:");
                                            ui.add(
                                                egui::DragValue::new(height)
                                                    .speed(0.05)
                                                    .clamp_range(0.01..=100.0),
                                            );
                                        });
                                    }
                                }
                            }
                            if remove_collider {
                                entity.collider = None;
                                self.is_dirty = true;
                            }

                            // 3EG. RigidBody Component
                            let mut remove_rigidbody = false;
                            if let Some(rb) = &mut entity.rigidbody {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.heading("📦 RigidBody Component");
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .button("🗑")
                                                .on_hover_text("Remove RigidBody")
                                                .clicked()
                                            {
                                                remove_rigidbody = true;
                                            }
                                        },
                                    );
                                });
                                ui.checkbox(&mut rb.active, "Active");
                                ui.checkbox(&mut rb.is_kinematic, "Is Kinematic");
                                ui.checkbox(&mut rb.use_gravity, "Use Gravity");

                                ui.horizontal(|ui| {
                                    ui.label("Mass:");
                                    ui.add(
                                        egui::DragValue::new(&mut rb.mass)
                                            .speed(0.05)
                                            .clamp_range(0.01..=1000.0),
                                    );
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Velocity:");
                                    ui.add(egui::DragValue::new(&mut rb.velocity.x).speed(0.1));
                                    ui.add(egui::DragValue::new(&mut rb.velocity.y).speed(0.1));
                                    ui.add(egui::DragValue::new(&mut rb.velocity.z).speed(0.1));
                                });
                            }
                            if remove_rigidbody {
                                entity.rigidbody = None;
                                self.is_dirty = true;
                            }

                            // 3EH. NavMeshAgent Component
                            let mut remove_nav_agent = false;
                            if let Some(agent) = &mut entity.nav_agent {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.heading("🛰️ NavMesh Agent Component");
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .button("🗑")
                                                .on_hover_text("Remove NavMesh Agent")
                                                .clicked()
                                            {
                                                remove_nav_agent = true;
                                            }
                                        },
                                    );
                                });
                                ui.checkbox(&mut agent.active, "Active");

                                ui.horizontal(|ui| {
                                    ui.label("Speed:");
                                    ui.add(
                                        egui::DragValue::new(&mut agent.speed)
                                            .speed(0.05)
                                            .clamp_range(0.0..=100.0),
                                    );
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Acceleration:");
                                    ui.add(
                                        egui::DragValue::new(&mut agent.acceleration)
                                            .speed(0.05)
                                            .clamp_range(0.0..=100.0),
                                    );
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Stopping Distance:");
                                    ui.add(
                                        egui::DragValue::new(&mut agent.stopping_distance)
                                            .speed(0.05)
                                            .clamp_range(0.0..=50.0),
                                    );
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Radius:");
                                    ui.add(
                                        egui::DragValue::new(&mut agent.radius)
                                            .speed(0.05)
                                            .clamp_range(0.01..=10.0),
                                    );
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Target:");
                                    ui.add(egui::DragValue::new(&mut agent.target.x).speed(0.1));
                                    ui.add(egui::DragValue::new(&mut agent.target.y).speed(0.1));
                                    ui.add(egui::DragValue::new(&mut agent.target.z).speed(0.1));
                                });

                                ui.horizontal(|ui| {
                                    ui.label(format!(
                                        "Velocity: [{:.2}, {:.2}, {:.2}]",
                                        agent.velocity.x, agent.velocity.y, agent.velocity.z
                                    ));
                                });
                            }
                            if remove_nav_agent {
                                entity.nav_agent = None;
                                self.is_dirty = true;
                            }

                            // Camera Component panel
                            let mut remove_camera = false;
                            if let Some(cam) = &mut entity.camera {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.heading("🎥 Camera Component");
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .button("🗑")
                                                .on_hover_text("Remove Camera")
                                                .clicked()
                                            {
                                                remove_camera = true;
                                            }
                                        },
                                    );
                                });

                                // Force high-quality motion blur by default under the hood
                                cam.motion_blur_active = true;
                                cam.motion_blur_samples = 64;

                                ui.horizontal(|ui| {
                                    ui.label("FOV:");
                                    ui.add(egui::Slider::new(&mut cam.fov, 1.0..=120.0));
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Near Clip:");
                                    ui.add(
                                        egui::DragValue::new(&mut cam.near)
                                            .speed(0.01)
                                            .clamp_range(0.01..=10.0),
                                    );
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Far Clip:");
                                    ui.add(
                                        egui::DragValue::new(&mut cam.far)
                                            .speed(1.0)
                                            .clamp_range(1.0..=1000.0),
                                    );
                                });

                                ui.colored_label(
                                    egui::Color32::from_rgb(0, 242, 254),
                                    "✔ Intrinsic Motion Blur (Active | 64 Samples)",
                                );
                            }
                            if remove_camera {
                                entity.camera = None;
                                entity.visual_correction = None;
                                self.is_dirty = true;
                            }

                            // Visual Correction Component panel
                            let mut remove_vc = false;
                            if let Some(vc) = &mut entity.visual_correction {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.heading("🎨 Visual Correction Component");
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .button("🗑")
                                                .on_hover_text("Remove Visual Correction")
                                                .clicked()
                                            {
                                                remove_vc = true;
                                            }
                                        },
                                    );
                                });

                                ui.checkbox(&mut vc.active, "Enable Visual Correction");

                                ui.add_space(3.0);
                                ui.label("✨ Bloom");
                                ui.checkbox(&mut vc.bloom_active, "  Bloom Active");
                                if vc.bloom_active {
                                    ui.horizontal(|ui| {
                                        ui.label("    Intensity:");
                                        ui.add(egui::Slider::new(
                                            &mut vc.bloom_intensity,
                                            0.0..=5.0,
                                        ));
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("    Threshold:");
                                        ui.add(egui::Slider::new(
                                            &mut vc.bloom_threshold,
                                            0.0..=1.0,
                                        ));
                                    });
                                }

                                ui.add_space(3.0);
                                ui.label("🌈 Color Correction");
                                ui.horizontal(|ui| {
                                    ui.label("  Exposure (EV):");
                                    ui.add(egui::Slider::new(&mut vc.exposure, -4.0..=4.0));
                                });
                                ui.horizontal(|ui| {
                                    ui.label("  Contrast:");
                                    ui.add(egui::Slider::new(&mut vc.contrast, 0.0..=2.0));
                                });
                                ui.horizontal(|ui| {
                                    ui.label("  Saturation:");
                                    ui.add(egui::Slider::new(&mut vc.saturation, 0.0..=2.0));
                                });

                                ui.add_space(3.0);
                                ui.label("🪞 Screen Space Reflections (SSR)");
                                ui.checkbox(&mut vc.ssr_active, "  SSR Active");
                                if vc.ssr_active {
                                    ui.horizontal(|ui| {
                                        ui.label("    Quality:");
                                        egui::ComboBox::from_label("")
                                            .selected_text(&vc.ssr_quality)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(
                                                    &mut vc.ssr_quality,
                                                    "Low".to_string(),
                                                    "Low",
                                                );
                                                ui.selectable_value(
                                                    &mut vc.ssr_quality,
                                                    "Medium".to_string(),
                                                    "Medium",
                                                );
                                                ui.selectable_value(
                                                    &mut vc.ssr_quality,
                                                    "High".to_string(),
                                                    "High",
                                                );
                                                ui.selectable_value(
                                                    &mut vc.ssr_quality,
                                                    "Ultra".to_string(),
                                                    "Ultra",
                                                );
                                            });
                                    });
                                    ui.checkbox(
                                        &mut vc.ssr_temporal_upsampling,
                                        "    Temporal Upsampling",
                                    );
                                }
                            }
                            if remove_vc {
                                entity.visual_correction = None;
                                self.is_dirty = true;
                            }

                            // 3F. Add Component Option
                            ui.separator();
                            ui.menu_button("➕ Add Component", |ui| {
                                if entity.script.is_none() {
                                    if ui.button("Script Component").clicked() {
                                        entity.script = Some(ScriptComponent {
                                            path: "project/assets/scripts/bot.lua".to_string(),
                                            is_loaded: false,
                                        });
                                        ui.close_menu();
                                    }
                                }
                                if entity.light.is_none() {
                                    if ui.button("💡 Light Component").clicked() {
                                        entity.light = Some(LightComponent {
                                            light_type: LightType::Point,
                                            color: Vec3::ONE,
                                            intensity: 1.5,
                                            range: 10.0,
                                            inner_cone: 30.0,
                                            outer_cone: 45.0,
                                        });
                                        ui.close_menu();
                                    }
                                }
                                if entity.health.is_none() {
                                    if ui.button("Health Component (Enemies)").clicked() {
                                        entity.health = Some(HealthComponent {
                                            current_health: 100.0,
                                            max_health: 100.0,
                                            is_dead: false,
                                        });
                                        entity.animator = Some(AnimatorComponent {
                                            current_clip: "Idle".to_string(),
                                            time: 0.0,
                                            speed: 2.0,
                                            is_playing: true,
                                            freeze: false,
                                        });
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
                                            color: [1.0, 1.0, 1.0],
                                        });
                                        ui.close_menu();
                                    }
                                }
                                if entity.nav_agent.is_none() {
                                    if ui.button("NavMesh Agent Component").clicked() {
                                        entity.nav_agent = Some(NavMeshAgentComponent {
                                            active: true,
                                            radius: 0.5,
                                            target: Vec3::new(8.0, 1.0, 8.0),
                                            speed: 3.0,
                                            acceleration: 5.0,
                                            stopping_distance: 0.5,
                                            velocity: Vec3::ZERO,
                                        });
                                        ui.close_menu();
                                    }
                                }
                                if entity.camera.is_none() {
                                    if ui.button("📷 Camera Component").clicked() {
                                        entity.camera = Some(CameraComponent {
                                            active: true,
                                            fov: 45.0,
                                            near: 0.1,
                                            far: 200.0,
                                            motion_blur_active: true,
                                            motion_blur_samples: 64,
                                        });
                                        ui.close_menu();
                                    }
                                }
                                if entity.camera.is_some() && entity.visual_correction.is_none() {
                                    if ui.button("🎨 Visual Correction Component").clicked() {
                                        entity.visual_correction =
                                            Some(VisualCorrectionComponent {
                                                active: true,
                                                bloom_active: true,
                                                bloom_intensity: 1.0,
                                                bloom_threshold: 0.8,
                                                exposure: 0.0,
                                                contrast: 1.0,
                                                saturation: 1.0,
                                                ssr_active: true,
                                                ssr_quality: "High".to_string(),
                                                ssr_temporal_upsampling: true,
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
                    } else if let Some(asset_path) = self.selected_asset_path.clone() {
                        // Modular Asset Inspectors dispatch call
                        inspectors::draw_inspector(ui, self, scene, console, &asset_path);
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
                        ui.colored_label(
                            egui::Color32::from_rgb(120, 120, 140),
                            "Provide a path to a panoramic image\n(e.g. assets/textures/sky.png)",
                        );
                        ui.add_space(8.0);

                        // 2. Ambient Light Color
                        ui.label("Ambient Color:");
                        ui.horizontal(|ui| {
                            let mut color_arr = [
                                scene.ambient_color.x,
                                scene.ambient_color.y,
                                scene.ambient_color.z,
                            ];
                            if ui.color_edit_button_rgb(&mut color_arr).changed() {
                                scene.ambient_color =
                                    Vec3::new(color_arr[0], color_arr[1], color_arr[2]);
                                self.is_dirty = true;
                            }
                        });
                        ui.add_space(8.0);

                        // 3. Ambient Light Intensity
                        ui.label("Ambient Intensity:");
                        let response = ui.add(
                            egui::Slider::new(&mut scene.ambient_intensity, 0.0..=5.0)
                                .text("intensity"),
                        );
                        if response.changed() {
                            self.is_dirty = true;
                        }

                        ui.add_space(15.0);
                        ui.separator();
                        ui.add_space(5.0);
                        ui.colored_label(
                            egui::Color32::from_rgb(100, 100, 130),
                            "Select an entity from Hierarchy\nor click a project asset to inspect.",
                        );
                    }
                });
            });

        // 4. BOTTOM PANEL: Folder Explorer & Console Logs
        egui::TopBottomPanel::bottom("Bottom Panel")
            .min_height(160.0)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(14, 14, 20))
                    .inner_margin(8.0)
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(30, 30, 40))),
            )
            .show(ctx, |ui| {
                // Tab Header Bar
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.active_bottom_tab == "assets", "📁 Project Assets")
                        .clicked()
                    {
                        self.active_bottom_tab = "assets".to_string();
                    }
                    if ui
                        .selectable_label(
                            self.active_bottom_tab == "console",
                            "📟 Developer Console",
                        )
                        .clicked()
                    {
                        self.active_bottom_tab = "console".to_string();
                    }

                    // Align dynamic tab utility button on the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.active_bottom_tab == "console" {
                            if ui.button("🗑 Clear Console").clicked() {
                                console.messages.clear();
                            }
                        } else {
                            if ui.button("⟲ Root").clicked() {
                                self.current_dir = "project".to_string();
                            }
                        }
                    });
                });
                ui.separator();
                ui.add_space(3.0);

                if self.active_bottom_tab == "assets" {
                    // Draw Breadcrumb Bar
                    let mut new_dir = None;
                    ui.horizontal(|ui| {
                        let parts: Vec<&str> = self
                            .current_dir
                            .split('/')
                            .filter(|s| !s.is_empty())
                            .collect();
                        let mut path_acc = String::new();
                        for (i, part) in parts.iter().enumerate() {
                            if i > 0 {
                                ui.label(">");
                            }
                            if !path_acc.is_empty() {
                                path_acc.push('/');
                            }
                            path_acc.push_str(part);

                            let current_path = path_acc.clone();
                            if ui
                                .selectable_label(self.current_dir == current_path, *part)
                                .clicked()
                            {
                                new_dir = Some(current_path);
                            }
                        }
                    });
                    if let Some(dir) = new_dir {
                        self.current_dir = dir;
                    }
                    ui.add_space(5.0);

                    egui::ScrollArea::vertical()
                        .id_source("FolderExplorerScroll")
                        .max_height(120.0)
                        .show(ui, |ui| {
                            if let Ok(entries) = std::fs::read_dir(&self.current_dir) {
                                let mut entries_list = Vec::new();

                                for entry in entries.flatten() {
                                    let path = entry.path();
                                    let path_str =
                                        path.to_string_lossy().to_string().replace("\\", "/");
                                    let is_dir = path.is_dir();
                                    entries_list.push((path_str, is_dir));
                                }

                                // Sort entries alphabetically by file/folder name
                                entries_list.sort_by(|a, b| {
                                    let name_a = Path::new(&a.0)
                                        .file_name()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("")
                                        .to_lowercase();
                                    let name_b = Path::new(&b.0)
                                        .file_name()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("")
                                        .to_lowercase();
                                    name_a.cmp(&name_b)
                                });

                                // Render parent directory ".." option if not at the root
                                if self.current_dir != "project" {
                                    if let Some(parent) = Path::new(&self.current_dir).parent() {
                                        let parent_str =
                                            parent.to_string_lossy().to_string().replace("\\", "/");
                                        ui.horizontal(|ui| {
                                            if ui.selectable_label(false, "📁 .. (Go Up)").clicked()
                                            {
                                                self.current_dir = parent_str;
                                            }
                                        });
                                    }
                                }

                                for (path_str, is_dir) in entries_list {
                                    let filename = Path::new(&path_str)
                                        .file_name()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("File")
                                        .to_string();
                                    if is_dir {
                                        ui.horizontal(|ui| {
                                            if ui
                                                .selectable_label(false, format!("📁 {}", filename))
                                                .clicked()
                                            {
                                                self.current_dir = path_str;
                                            }
                                        });
                                    } else {
                                        let extension = Path::new(&path_str)
                                            .extension()
                                            .and_then(|s| s.to_str())
                                            .unwrap_or("")
                                            .to_lowercase();
                                        let icon = match extension.as_str() {
                                            "png" | "tga" | "jpg" | "jpeg" => "🖼️",
                                            "wav" | "mp3" | "ogg" => "🎵",
                                            "scene" => "🎬",
                                            "fbx" | "obj" => "📐",
                                            "lua" => "📄",
                                            _ => "📝",
                                        };

                                        let is_selected =
                                            self.selected_asset_path.as_ref() == Some(&path_str);

                                        ui.horizontal(|ui| {
                                            let label_res = ui.selectable_label(
                                                is_selected,
                                                format!("{} {}", icon, filename),
                                            );
                                            if label_res.clicked() {
                                                self.selected_asset_path = Some(path_str.clone());
                                                self.selected_entity_id = None; // Deselect scene entity!

                                                // Load initial data for script editing
                                                if extension == "lua" {
                                                    if let Ok(content) =
                                                        std::fs::read_to_string(&path_str)
                                                    {
                                                        self.asset_script_content = content;
                                                    }
                                                }
                                            }
                                        });
                                    }
                                }
                            } else {
                                ui.colored_label(egui::Color32::RED, "Failed to read directory.");
                            }
                        });
                } else if self.active_bottom_tab == "console" {
                    egui::ScrollArea::vertical()
                        .id_source("ConsoleScroll")
                        .max_height(120.0)
                        .show(ui, |ui| {
                            if console.messages.is_empty() {
                                ui.colored_label(
                                    egui::Color32::from_rgb(100, 100, 130),
                                    "  No execution logs yet. Logs will print when running.",
                                );
                            } else {
                                for (msg, level) in &console.messages {
                                    let color = match level {
                                        LogLevel::Info => egui::Color32::from_rgb(220, 220, 230),
                                        LogLevel::Warning => egui::Color32::from_rgb(255, 200, 50),
                                        LogLevel::Error => egui::Color32::from_rgb(255, 60, 100),
                                    };
                                    ui.colored_label(color, format!("  {}", msg));
                                }
                            }
                        });
                }
            });
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

        for &child_id in &entity.children {
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
}
