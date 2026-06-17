use egui_phosphor::regular as icon;

use crate::editor::{
    inspector_add, inspector_camera, inspector_gameplay, inspector_particles, inspector_render,
    inspector_settings, inspector_transform,
};
use crate::editor::{inspectors, EditorUi, InspectorTarget};
use crate::navigation::NavigationGraph;
use crate::scene::{Entity, Scene, LAYER_COUNT};
use crate::scripting::ConsoleLogs;

/// RIGHT PANEL: Properties Inspector
pub fn draw(
    editor: &mut EditorUi,
    ctx: &egui::Context,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    nav: &mut NavigationGraph,
) {
    let t = editor.theme;
    if !editor.inspector_open {
        draw_collapsed(ctx, t, &mut editor.inspector_open);
        return;
    }
    egui::SidePanel::right("Inspector Panel")
        .resizable(true)
        .width_range(182.0..=380.0)
        .frame(
            egui::Frame::none()
                .fill(t.bg_tier1)
                .inner_margin(t.space_md)
                .stroke(egui::Stroke::new(1.0, t.border)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(format!("{}  Inspector", icon::SLIDERS_HORIZONTAL));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(icon::CARET_RIGHT)
                        .on_hover_text("Collapse")
                        .clicked()
                    {
                        editor.inspector_open = false;
                    }
                });
            });
            ui.separator();
            ui.add_space(5.0);

            egui::ScrollArea::vertical().show(ui, |ui| {
                // Dispatch on the explicit inspector target (mutually exclusive).
                match editor.inspector_target() {
                    InspectorTarget::Entity(selected_id) => {
                        draw_entity_inspector(editor, ui, scene, nav, selected_id);
                    }
                    InspectorTarget::Asset(asset_path) => {
                        inspectors::draw_inspector(ui, editor, scene, console, &asset_path);
                    }
                    InspectorTarget::SceneSettings => {
                        inspector_settings::draw(editor, ui, scene);
                    }
                }
            });
        });
}

/// Collapsed state: a thin rail with a caret that reopens the inspector panel.
fn draw_collapsed(ctx: &egui::Context, t: crate::editor::theme::Theme, open: &mut bool) {
    egui::SidePanel::right("Inspector Rail")
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
                .button(icon::CARET_LEFT)
                .on_hover_text("Expand")
                .clicked()
            {
                *open = true;
            }
        });
}

/// Read-only context the inspector needs that borrows the whole `Scene`, gathered
/// up front because the entity guard below borrows it mutably for the rest of the
/// frame.
struct InspectorContext {
    layer_labels: Vec<String>,
    named_layers: Vec<(u8, String)>,
    parent_mat: Option<glam::Mat4>,
    selected_parent_name: String,
    valid_parents: Vec<(u32, String)>,
}

fn draw_entity_inspector(
    editor: &mut EditorUi,
    ui: &mut egui::Ui,
    scene: &mut Scene,
    nav: &mut NavigationGraph,
    selected_id: u32,
) {
    let mut pending_parent_change = None;
    let mut pending_nav_bake = false;
    let mut layer_changed = false;

    let cx = gather_context(scene, selected_id);

    if let Some(mut entity_guard) = scene.get_entity_mut(selected_id) {
        let entity: &mut Entity = &mut entity_guard;
        if entity.camera.is_none() && entity.visual_correction.is_some() {
            entity.visual_correction = None;
        }

        draw_object_header(
            ui,
            editor.theme,
            entity,
            &cx.layer_labels,
            &mut pending_nav_bake,
            &mut layer_changed,
        );

        inspector_transform::draw(
            ui,
            entity,
            cx.parent_mat,
            &cx.selected_parent_name,
            &cx.valid_parents,
            &mut pending_parent_change,
            &mut pending_nav_bake,
            &mut editor.is_dirty,
        );

        draw_components(
            ui,
            entity,
            &cx.named_layers,
            &mut editor.is_dirty,
            &mut pending_nav_bake,
        );
    }

    if layer_changed {
        editor.is_dirty = true;
    }
    if let Some(new_parent) = pending_parent_change {
        let _ = scene.set_parent(selected_id, new_parent);
    }
    if pending_nav_bake {
        nav.bake(scene);
    }
}

/// Gather the read-only layer labels, parenting matrix/name, and valid-parent set
/// for the selected entity (see [`InspectorContext`]).
fn gather_context(scene: &Scene, selected_id: u32) -> InspectorContext {
    // The layer dropdown labels (one per registry slot).
    let layer_labels: Vec<String> = (0..LAYER_COUNT)
        .map(|i| scene.layers.label(i as u8))
        .collect();

    // Layers offered in the camera culling-mask checklist: layer 0 plus any named
    // user slots (unnamed slots are noise, mirroring Unity's mask dropdown).
    let named_layers: Vec<(u8, String)> = (0..LAYER_COUNT as u8)
        .filter(|&i| i == 0 || !scene.layers.name(i).is_empty())
        .map(|i| (i, scene.layers.label(i)))
        .collect();

    let current_parent_id = scene.get_entity(selected_id).and_then(|e| e.parent_id);
    let parent_mat = current_parent_id.map(|p| scene.compute_world_matrix(p));
    let selected_parent_name = if let Some(p_id) = current_parent_id {
        scene
            .get_entity(p_id)
            .map(|e| e.name.clone())
            .unwrap_or("None".to_string())
    } else {
        "None".to_string()
    };

    InspectorContext {
        layer_labels,
        named_layers,
        parent_mat,
        selected_parent_name,
        valid_parents: collect_valid_parents(scene, selected_id),
    }
}

/// The entities that may be the selected entity's parent: everything except
/// itself and its own descendants (which would create a cycle).
fn collect_valid_parents(scene: &Scene, selected_id: u32) -> Vec<(u32, String)> {
    scene
        .iter()
        .filter(|e| e.id != selected_id)
        .map(|e| (e.id, e.name.clone()))
        .filter(|(id, _)| {
            let mut curr = *id;
            while let Some(ancestor) = scene.get_entity(curr).and_then(|x| x.parent_id) {
                if ancestor == selected_id {
                    return false;
                }
                curr = ancestor;
            }
            true
        })
        .collect()
}

/// The Unity-style object header card: name + active/static toggles + layer combo.
fn draw_object_header(
    ui: &mut egui::Ui,
    t: crate::editor::theme::Theme,
    entity: &mut Entity,
    layer_labels: &[String],
    pending_nav_bake: &mut bool,
    layer_changed: &mut bool,
) {
    egui::Frame::none()
        .fill(t.bg_tier2)
        .inner_margin(t.space_sm)
        .rounding(4.0)
        .stroke(egui::Stroke::new(1.0, t.border))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(t.text_secondary, icon::CUBE_FOCUS);
                ui.add(egui::TextEdit::singleline(&mut entity.name).desired_width(f32::INFINITY));
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut entity.active, "Active");
                if ui
                    .checkbox(&mut entity.is_static, "Static")
                    .on_hover_text("Blocks the navmesh")
                    .changed()
                {
                    *pending_nav_bake = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Layer:");
                let selected = layer_labels
                    .get(entity.layer as usize)
                    .cloned()
                    .unwrap_or_default();
                egui::ComboBox::from_id_source("entity_layer")
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        for (i, label) in layer_labels.iter().enumerate() {
                            if ui
                                .selectable_value(&mut entity.layer, i as u8, label)
                                .clicked()
                            {
                                *layer_changed = true;
                            }
                        }
                    });
            });
        });
    ui.add_space(t.space_xs);
}

/// The per-component card chain (mesh, material, light, gameplay, camera, …) plus
/// the Add Component menu, in the canonical inspector order.
fn draw_components(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    named_layers: &[(u8, String)],
    is_dirty: &mut bool,
    pending_nav_bake: &mut bool,
) {
    inspector_render::draw_mesh(ui, entity, is_dirty);
    inspector_render::draw_texture(ui, entity, is_dirty);
    inspector_render::draw_light(ui, entity, is_dirty);

    inspector_gameplay::draw_script(ui, entity, is_dirty);
    inspector_gameplay::draw_health(ui, entity, is_dirty);
    inspector_gameplay::draw_collider(ui, entity, is_dirty, pending_nav_bake);
    inspector_gameplay::draw_rigidbody(ui, entity, is_dirty);
    inspector_gameplay::draw_nav_agent(ui, entity, is_dirty);

    inspector_camera::draw_camera(ui, entity, named_layers, is_dirty);
    inspector_camera::draw_visual_correction(ui, entity, is_dirty);

    inspector_particles::draw(ui, entity, is_dirty);

    inspector_add::draw(ui, entity);
}
