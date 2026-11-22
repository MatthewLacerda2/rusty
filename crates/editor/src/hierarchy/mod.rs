//! The Scene Hierarchy: a collapsible tree flattened into rows (`flat`) and drawn
//! only where it is in view (`tree`). Object creation lives in the menu bar's
//! GameObject menu; deleting is Unity's: Edit ▸ Delete, the Delete key
//! (Cmd+Backspace on macOS) and a row's right-click *Delete*, all through `delete`.

use egui_phosphor::regular as icon;

use crate::theme::chrome;
use crate::EditorUi;
use rusty_scene::Scene;

mod flat;
mod glyph;
#[cfg(test)]
mod tests;
mod tree;
#[cfg(test)]
mod width_tests;

use flat::Flattener;

/// The panel's egui id, under which egui keeps the width the user dragged.
const PANEL_ID: &str = "Hierarchy Panel";

/// The panel's width, margins included, until the user drags its edge.
const DEFAULT_WIDTH: f32 = 220.0;

pub fn draw(editor: &mut EditorUi, ui: &mut egui::Ui, scene: &mut Scene) {
    let t = editor.theme;
    if !editor.hierarchy_open {
        draw_collapsed(ui, t, &mut editor.hierarchy_open);
        return;
    }
    let mut doomed = None;
    let frame = chrome::panel_frame(&t);
    egui::Panel::left(PANEL_ID)
        .resizable(true)
        .size_range((154.0 + frame.inner_margin.sum().x)..=340.0)
        .default_size(DEFAULT_WIDTH)
        .frame(frame)
        .show(ui, |ui| {
            let title = (icon::TREE_STRUCTURE, "Hierarchy");
            if chrome::panel_header(ui, &t, title.0, title.1, icon::CARET_LEFT) {
                editor.hierarchy_open = false;
            }
            // The walk's buffers live in egui's memory: a warm frame allocates none.
            let kept = egui::Id::new("hierarchy_flattener");
            let mut flat =
                ui.data_mut(|d| std::mem::take(d.get_temp_mut_or_default::<Flattener>(kept)));
            flat.flatten(ui.ctx(), scene);
            let mut rows = tree::Rows {
                sel_entity: &mut editor.selected_entity_id,
                sel_asset: &mut editor.selected_asset_path,
                delete: &mut doomed,
                theme: t,
            };
            rows.draw(ui, scene, &flat.rows);
            ui.data_mut(|d| d.insert_temp(kept, flat));
        });
    if let Some(id) = doomed {
        delete(editor, scene, id);
    }
}

pub(crate) fn delete(editor: &mut EditorUi, scene: &mut Scene, id: u32) {
    if !scene.world.contains(id) {
        return;
    }
    scene.destroy(id);
    editor.selected_entity_id = editor
        .selected_entity_id
        .filter(|&sel| scene.world.contains(sel));
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
