//! A loaded scene's group header (Unity's multi-scene hierarchy): its name, bold when active,
//! `*` when its file holds unsaved edits, and a context menu to make it active, save or unload.

use egui::collapsing_header::{paint_default_icon, CollapsingState};
use egui_phosphor::regular as icon;

use super::flat::{group_id, Group};
use super::marks::SceneMarks;
use crate::EditorUi;
use rusty_scene::Scene;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SceneAction {
    SetActive,
    Save,
    Unload,
}

pub(super) fn header(
    ui: &mut egui::Ui,
    scene: &Scene,
    group: Group,
    marks: &SceneMarks,
    action: &mut Option<(u32, SceneAction)>,
) {
    let mut state = CollapsingState::load_with_default_open(ui.ctx(), group_id(group), true);
    if state.show_toggle_button(ui, paint_default_icon).clicked() {
        state.store(ui.ctx());
    }
    let Group::Scene(handle) = group else {
        ui.label(
            egui::RichText::new(rusty_scene::runtime::membership::DONT_DESTROY_ON_LOAD).strong(),
        );
        return;
    };
    let path = scene.loaded.path_of(handle);
    let mut name = stem(path);
    if path.is_some_and(|p| marks.is_unsaved(p)) || path.is_none() {
        name.push('*');
    }
    let active = scene.loaded.active() == handle;
    let text = egui::RichText::new(format!("{}  {name}", icon::FILE));
    let text = match active {
        true => {
            let size = egui::TextStyle::Body.resolve(ui.style()).size;
            text.font(crate::theme::fonts::semibold(ui.ctx(), size))
        }
        false => text,
    };
    let label = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
    label.context_menu(|ui| {
        let items = [
            (SceneAction::SetActive, "Set Active Scene", !active),
            (SceneAction::Save, "Save Scene", path.is_some()),
            (
                SceneAction::Unload,
                "Unload Scene",
                scene.loaded.handles().len() > 1,
            ),
        ];
        for (verb, title, enabled) in items {
            if ui.add_enabled(enabled, egui::Button::new(title)).clicked() {
                *action = Some((handle, verb));
                ui.close();
            }
        }
    });
}

fn stem(path: Option<&str>) -> String {
    let stem = path.and_then(|p| std::path::Path::new(p).file_stem());
    stem.map_or_else(|| "Untitled".into(), |s| s.to_string_lossy().into_owned())
}

/// Each reports a failure on the console under the editor's log target.
pub(super) fn act(editor: &mut EditorUi, scene: &mut Scene, handle: u32, action: SceneAction) {
    let path = scene.loaded.path_of(handle).map(str::to_string);
    match action {
        SceneAction::SetActive => {
            let now = scene.set_active_scene(handle).filter(|p| !p.is_empty());
            editor.set_current_scene_path(now);
        }
        SceneAction::Save => {
            let Some(path) = path else { return };
            let saved = rusty_edit::save_to_file(scene, &path);
            if crate::report::failed("Save Scene", saved).is_some() {
                editor.scene_marks.saved();
                log::info!("[Editor] Scene saved to {path}");
            }
        }
        SceneAction::Unload => {
            let unsaved = path
                .as_deref()
                .is_some_and(|p| editor.scene_marks.is_unsaved(p));
            let Some(now) = crate::report::failed("Unload Scene", scene.unload_scene(handle))
            else {
                return;
            };
            if let Some(path) = &path {
                scene.forget_disk_text_at(path);
                if unsaved {
                    log::warn!("[Editor] {path} was unloaded with unsaved edits; they are gone");
                }
            }
            editor.set_current_scene_path(now);
            // Its edits left with it: the mark is the remaining files'.
            editor.is_dirty = rusty_edit::scenes::any_unsaved(scene);
            editor.selected_entity_id = editor
                .selected_entity_id
                .filter(|&id| scene.world.contains(id));
        }
    }
}
