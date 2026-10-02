use egui_phosphor::regular as icon;

use crate::editor::theme::chrome;
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
        .frame(chrome::panel_frame(&t))
        .show(ctx, |ui| {
            let title = (icon::TREE_STRUCTURE, "Hierarchy");
            if chrome::panel_header(ui, &t, title.0, title.1, icon::CARET_LEFT) {
                editor.hierarchy_open = false;
            }
            draw_actions(editor, scene, ui);

            egui::ScrollArea::vertical().show(ui, |ui| {
                let root_ids: Vec<u32> = scene
                    .entity_ids()
                    .into_iter()
                    .filter(|&id| scene.world.parent_id(id).is_none())
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
        .frame(chrome::rail_frame(&t))
        .show(ctx, |ui| {
            if chrome::icon_button(ui, icon::CARET_RIGHT, "Expand").clicked() {
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
        chrome::hairline(ui, &editor.theme);
    }
}
