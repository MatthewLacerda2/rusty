//! LEFT PANEL: Scene Hierarchy — a VS Code Explorer-style collapsible tree
//! (`tree`). Object creation lives in the menu bar's GameObject menu (#255);
//! deleting is Unity's (#1915): Edit ▸ Delete, the Delete key (Cmd+Backspace on
//! macOS) and a row's right-click *Delete*, all through `delete`.

use egui_phosphor::regular as icon;

use crate::theme::chrome;
use crate::EditorUi;
use rusty_scene::Scene;

mod tree;

/// Draw the Hierarchy panel, or its collapsed rail.
pub fn draw(editor: &mut EditorUi, ui: &mut egui::Ui, scene: &mut Scene) {
    let t = editor.theme;
    if !editor.hierarchy_open {
        draw_collapsed(ui, t, &mut editor.hierarchy_open);
        return;
    }
    let mut doomed = None;
    let frame = chrome::panel_frame(&t);
    egui::Panel::left("Hierarchy Panel")
        .resizable(true)
        .size_range((154.0 + frame.inner_margin.sum().x)..=340.0)
        .frame(frame)
        .show(ui, |ui| {
            let title = (icon::TREE_STRUCTURE, "Hierarchy");
            if chrome::panel_header(ui, &t, title.0, title.1, icon::CARET_LEFT) {
                editor.hierarchy_open = false;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                let root_ids: Vec<u32> = scene
                    .entity_ids()
                    .into_iter()
                    .filter(|&id| scene.world.parent_id(id).is_none())
                    .collect();
                let mut rows = tree::Rows {
                    sel_entity: &mut editor.selected_entity_id,
                    sel_asset: &mut editor.selected_asset_path,
                    delete: &mut doomed,
                    theme: t,
                };
                for entity_id in root_ids {
                    rows.draw_node(ui, scene, entity_id);
                }
            });
        });
    if let Some(id) = doomed {
        delete(editor, scene, id);
    }
}

/// Delete `id` and its whole subtree, as Unity's Edit ▸ Delete does: the one
/// path behind the menu item, the Delete key and the row's context menu, so the
/// three cannot drift. In edit mode it removes at once and the frame's Undo step
/// records it (`shell::editor::undo`); the selection is cleared when it was in
/// the subtree, and the scene has unsaved changes.
pub(crate) fn delete(editor: &mut EditorUi, scene: &mut Scene, id: u32) {
    if !scene.world.contains(id) {
        return;
    }
    scene.destroy(id);
    editor.selected_entity_id = editor
        .selected_entity_id
        .filter(|&sel| scene.world.contains(sel));
    editor.is_dirty = true;
}

/// Collapsed state: a thin rail with a caret that reopens the hierarchy panel.
fn draw_collapsed(ui: &mut egui::Ui, t: crate::theme::Theme, open: &mut bool) {
    let frame = chrome::rail_frame(&t);
    egui::Panel::left("Hierarchy Rail")
        .resizable(false)
        .exact_size(26.0 + frame.inner_margin.sum().x)
        .frame(frame)
        .show(ui, |ui| {
            if chrome::icon_button(ui, icon::CARET_RIGHT, "Expand").clicked() {
                *open = true;
            }
        });
}
