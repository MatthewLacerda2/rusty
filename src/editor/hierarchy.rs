use egui_phosphor::regular as icon;
use glam::Vec3;

use crate::editor::{hierarchy_tree, EditorUi};
use crate::scene::{LightComponent, LightType, MeshComponent, Scene};

/// LEFT PANEL: Scene Hierarchy — a VS Code Explorer-style collapsible tree with a
/// creation toolbar pinned at the top.
pub fn draw(editor: &mut EditorUi, ctx: &egui::Context, scene: &mut Scene) {
    let t = editor.theme;
    if !editor.hierarchy_open {
        draw_collapsed(ctx, t, &mut editor.hierarchy_open);
        return;
    }
    egui::SidePanel::left("Hierarchy Panel")
        .resizable(true)
        .width_range(154.0..=340.0)
        .frame(
            egui::Frame::none()
                .fill(t.bg_tier1)
                .inner_margin(t.space_md)
                .stroke(egui::Stroke::new(1.0, t.border)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(format!("{}  Scene Hierarchy", icon::TREE_STRUCTURE));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(icon::CARET_LEFT)
                        .on_hover_text("Collapse")
                        .clicked()
                    {
                        editor.hierarchy_open = false;
                    }
                });
            });
            draw_toolbar(editor, scene, ui);
            ui.separator();
            ui.add_space(t.space_xs);

            egui::ScrollArea::vertical().show(ui, |ui| {
                let root_ids: Vec<u32> = scene
                    .iter()
                    .filter(|e| e.parent_id.is_none())
                    .map(|e| e.id)
                    .collect();
                for entity_id in root_ids {
                    hierarchy_tree::draw_node(
                        ui,
                        scene,
                        entity_id,
                        &mut editor.selected_entity_id,
                        &mut editor.selected_asset_path,
                        t,
                    );
                }
            });
        });
}

/// Collapsed state: a thin rail with a caret that reopens the hierarchy panel.
fn draw_collapsed(ctx: &egui::Context, t: crate::editor::theme::Theme, open: &mut bool) {
    egui::SidePanel::left("Hierarchy Rail")
        .resizable(false)
        .exact_width(26.0)
        .frame(
            egui::Frame::none()
                .fill(t.bg_tier1)
                .inner_margin(t.space_xs)
                .stroke(egui::Stroke::new(1.0, t.border)),
        )
        .show(ctx, |ui| {
            if ui
                .button(icon::CARET_RIGHT)
                .on_hover_text("Expand")
                .clicked()
            {
                *open = true;
            }
        });
}

/// Creation toolbar: name + primitive type + Create, and Destroy for the selection.
/// Lives above the tree so the tree owns the whole scroll area.
fn draw_toolbar(editor: &mut EditorUi, scene: &mut Scene, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut editor.new_entity_name)
                .desired_width(110.0)
                .hint_text("Name"),
        );
        egui::ComboBox::from_id_source("new_entity_type")
            .selected_text(&editor.new_entity_type)
            .width(86.0)
            .show_ui(ui, |ui| {
                for (val, label) in [
                    ("Box", "Box Mesh"),
                    ("Sphere", "Sphere Mesh"),
                    ("Plane", "Plane Mesh"),
                    ("Cylinder", "Cylinder Mesh"),
                    ("PointLight", "Point Light"),
                    ("DirectionalLight", "Dir Light"),
                    ("SpotLight", "Spot Light"),
                ] {
                    ui.selectable_value(&mut editor.new_entity_type, val.to_string(), label);
                }
            });
    });
    ui.horizontal(|ui| {
        if ui.button(format!("{}  Create", icon::PLUS)).clicked() {
            create_entity(editor, scene);
        }
        if let Some(selected_id) = editor.selected_entity_id {
            if ui.button(format!("{}  Destroy", icon::TRASH)).clicked() {
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
            ent.mesh = Some(mesh_component("Box", v, idx));
        } else if name_lower == "sphere" {
            let (v, idx) = crate::render::mesh::generate_sphere(1.0, 16, 16);
            ent.mesh = Some(mesh_component("Sphere", v, idx));
        } else if name_lower == "plane" {
            let (v, idx) = crate::render::mesh::generate_plane(15.0, 15.0);
            ent.mesh = Some(mesh_component("Plane", v, idx));
        } else if name_lower == "cylinder" {
            let (v, idx) = crate::render::mesh::generate_cylinder(
                Vec3::new(0.0, -0.5, 0.0),
                Vec3::new(0.0, 0.5, 0.0),
                0.5,
                12,
            );
            ent.mesh = Some(mesh_component("Cylinder", v, idx));
        } else if name_lower == "pointlight" {
            ent.light = Some(light(LightType::Point, Vec3::ONE, 1.5, 10.0));
        } else if name_lower == "directionallight" {
            ent.light = Some(light(
                LightType::Directional,
                Vec3::new(1.0, 0.95, 0.8),
                2.0,
                0.0,
            ));
        } else if name_lower == "spotlight" {
            ent.light = Some(light(LightType::Spotlight, Vec3::ONE, 2.0, 15.0));
        }
        editor.selected_entity_id = Some(new_id);
        editor.selected_asset_path = None; // clear asset selection
        editor.is_dirty = true;
    }
}

fn mesh_component(
    kind: &str,
    vertices: Vec<crate::render::mesh::Vertex>,
    indices: Vec<u32>,
) -> MeshComponent {
    MeshComponent {
        primitive_type: kind.to_string(),
        vertices,
        indices,
        is_dirty: crate::scene::DirtyFlag::new(true),
    }
}

fn light(light_type: LightType, color: Vec3, intensity: f32, range: f32) -> LightComponent {
    LightComponent {
        light_type,
        color,
        intensity,
        range,
        inner_cone: 30.0,
        outer_cone: 45.0,
    }
}
