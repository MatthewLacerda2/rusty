use egui_phosphor::regular as icon;

use crate::editor::{hierarchy_tree, EditorUi};
use crate::scene::Scene;

/// LEFT PANEL: Scene Hierarchy — a VS Code Explorer-style collapsible tree. Object
/// creation lives in the menu bar's GameObject menu (#255); the panel keeps only a
/// Destroy affordance for the current selection above the tree.
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
            draw_actions(editor, scene, ui);
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

/// Selection action bar: a Destroy affordance for the current selection (creation
/// now lives in the menu bar's GameObject menu). Lives above the tree so the tree
/// owns the whole scroll area. Destroy only shows when something is selected.
fn draw_actions(editor: &mut EditorUi, scene: &mut Scene, ui: &mut egui::Ui) {
    if let Some(selected_id) = editor.selected_entity_id {
        if ui.button(format!("{}  Destroy", icon::TRASH)).clicked() {
            scene.destroy_entity(selected_id);
            editor.selected_entity_id = None;
            editor.is_dirty = true;
        }
    }
}
