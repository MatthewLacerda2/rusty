pub mod assets;
pub mod components;

use egui_phosphor::regular as icon;

use self::components::context::gather_context;
use self::components::{
    add, audio, camera, gameplay, material, particles, prefab, render, settings, transform,
};
use crate::editor::{EditorUi, InspectorTarget};
use crate::navigation::NavigationGraph;
use crate::scene::{Entity, MaterialAsset, Scene};
use crate::scripting::ConsoleLogs;
use std::collections::BTreeMap;

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
                        assets::draw_inspector(ui, editor, scene, console, nav, &asset_path);
                    }
                    InspectorTarget::SceneSettings => {
                        settings::draw(editor, ui, scene);
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

/// Edits the per-frame card draw defers until the mutable entity guard has dropped,
/// because each needs `&mut Scene` (or `&mut NavigationGraph`) that the guard's borrow
/// would otherwise block. Collected during the draw, then applied by [`Self::apply`].
#[derive(Default)]
struct PendingEdits {
    parent_change: Option<Option<u32>>,
    nav_bake: bool,
    layer_changed: bool,
    prefab_action: Option<prefab::PrefabAction>,
}

impl PendingEdits {
    /// Apply the deferred edits once the entity guard has dropped: re-parent, run a
    /// prefab verb, mark dirty, and re-bake the navmesh as requested.
    fn apply(self, editor: &mut EditorUi, scene: &mut Scene, nav: &mut NavigationGraph, id: u32) {
        if self.layer_changed {
            editor.is_dirty = true;
        }
        if let Some(action) = self.prefab_action {
            if prefab::dispatch(scene, action) {
                editor.is_dirty = true;
            }
        }
        if let Some(new_parent) = self.parent_change {
            let _ = scene.set_parent(id, new_parent);
        }
        if self.nav_bake {
            nav.bake(scene);
        }
    }
}

fn draw_entity_inspector(
    editor: &mut EditorUi,
    ui: &mut egui::Ui,
    scene: &mut Scene,
    nav: &mut NavigationGraph,
    selected_id: u32,
) {
    let mut pending = PendingEdits::default();
    let cx = gather_context(scene, selected_id);

    // Borrow the entity (`scene.world`) and the material library (`scene.materials`)
    // as DISJOINT fields of `Scene`, so the material card can edit the shared library
    // asset while the entity guard is live (they never alias).
    let materials = &mut scene.materials;
    if let Some(mut entity_guard) = scene.world.get_mut(selected_id) {
        let entity: &mut Entity = &mut entity_guard;
        if entity.camera.is_none() && entity.visual_correction.is_some() {
            entity.visual_correction = None;
        }

        draw_object_header(
            ui,
            editor.theme,
            entity,
            &cx.layer_labels,
            &mut pending.nav_bake,
            &mut pending.layer_changed,
        );

        // Linked-prefab override affordance, just under the header (a no-op for a plain
        // entity). Emits a deferred action so the prefab verbs run after the guard drops.
        pending.prefab_action = prefab::draw(ui, entity, cx.prefab_root);

        transform::draw(
            ui,
            entity,
            cx.parent_mat,
            &cx.selected_parent_name,
            &cx.valid_parents,
            &mut pending.parent_change,
            &mut pending.nav_bake,
            &mut editor.is_dirty,
        );

        draw_components(
            ui,
            entity,
            materials,
            &cx.named_layers,
            &mut editor.is_dirty,
            &mut pending.nav_bake,
        );
    }

    pending.apply(editor, scene, nav, selected_id);
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
    materials: &mut BTreeMap<String, MaterialAsset>,
    named_layers: &[(u8, String)],
    is_dirty: &mut bool,
    pending_nav_bake: &mut bool,
) {
    render::draw_mesh(ui, entity, is_dirty);
    material::draw_material_card(ui, entity, materials, is_dirty);
    render::draw_light(ui, entity, is_dirty);

    gameplay::draw_script(ui, entity, is_dirty);
    gameplay::draw_animator(ui, entity, is_dirty);
    gameplay::draw_collider(ui, entity, is_dirty, pending_nav_bake);
    gameplay::draw_rigidbody(ui, entity, is_dirty);
    gameplay::draw_nav_agent(ui, entity, is_dirty);

    camera::draw_camera(ui, entity, named_layers, is_dirty);
    camera::draw_visual_correction(ui, entity, is_dirty);

    particles::draw(ui, entity, is_dirty);
    audio::draw(ui, entity, is_dirty);

    add::draw(ui, entity);
}
