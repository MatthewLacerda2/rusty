//! The Edit menu and its shortcuts (#1631): Undo / Redo over the scene's history
//! (`scene::undo`), Unity's Edit ▸ Undo and Ctrl+Z / Ctrl+Shift+Z (Cmd on macOS),
//! and Edit ▸ Delete with the Delete key, Cmd+Backspace on macOS (#1915).

use egui::{Key, KeyboardShortcut, Modifiers};
use egui_phosphor::regular as icon;

use crate::{hierarchy, EditorUi};
use rusty_scene::Scene;

const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
/// Ctrl+Y, Unity's Redo on Windows and Linux.
const REDO_Y: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const DELETE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::Delete);
/// Cmd+Backspace, Unity's Delete on macOS, whose `delete` key is Backspace. It
/// names the Mac's own Cmd, so Ctrl+Backspace elsewhere is not a delete.
const DELETE_MAC: KeyboardShortcut = KeyboardShortcut::new(Modifiers::MAC_CMD, Key::Backspace);

/// The REPL line's widget id: its lines are Undo steps of their own.
pub(crate) fn repl_input_id() -> egui::Id {
    egui::Id::new("rusty.repl_input")
}

/// Whether a text field holds the open Undo step: one being typed into, so a
/// rename is one step however many frames it takes. Only a *text field* counts:
/// the viewport and a dragged slider take keyboard focus too and keep it after
/// the gesture, and the console keeps it after Enter, but its lines are their
/// own steps (`ScriptManager::eval`).
pub fn typing_holds_step(ctx: &egui::Context) -> bool {
    ctx.text_edit_focused() && ctx.memory(|m| m.focused()) != Some(repl_input_id())
}

/// Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y and Delete, in edit mode, unless a text field
/// has the keyboard: there they edit the text, as in Unity. Focus on any other
/// widget (the viewport after a camera gesture, a slider after its drag) does
/// not take the shortcut away.
pub(super) fn shortcuts(editor: &mut EditorUi, ctx: &egui::Context, scene: &mut Scene) {
    if scene.is_playing() || ctx.text_edit_focused() {
        return;
    }
    // Redo first: its shortcut is Undo's plus Shift, and egui matches Undo's
    // logically, Shift or not.
    let redo = ctx.input_mut(|i| i.consume_shortcut(&REDO) || i.consume_shortcut(&REDO_Y));
    if redo {
        step(editor, scene, false);
    } else if ctx.input_mut(|i| i.consume_shortcut(&UNDO)) {
        step(editor, scene, true);
    }
    let Some(selected) = editor.selected_entity_id else {
        return;
    };
    if ctx.input_mut(|i| i.consume_shortcut(&DELETE) || i.consume_shortcut(&DELETE_MAC)) {
        hierarchy::delete(editor, scene, selected);
    }
}

/// The Edit menu: Undo and Redo, each naming its step, then Delete for the
/// selection; greyed out in Play and when there is nothing to act on.
pub(super) fn edit_menu(editor: &mut EditorUi, ui: &mut egui::Ui, scene: &mut Scene) {
    let playing = scene.is_playing();
    ui.menu_button("Edit", |ui| {
        let undo = scene.undo_name().map(|n| format!("Undo {n}"));
        if item(ui, icon::ARROW_U_UP_LEFT, undo, "Undo", &UNDO, playing) {
            step(editor, scene, true);
            ui.close();
        }
        let redo = scene.redo_name().map(|n| format!("Redo {n}"));
        if item(ui, icon::ARROW_U_UP_RIGHT, redo, "Redo", &REDO, playing) {
            step(editor, scene, false);
            ui.close();
        }
        ui.separator();
        let selected = editor.selected_entity_id;
        let delete = selected.map(|_| "Delete".to_owned());
        if item(ui, icon::TRASH, delete, "Delete", &DELETE, playing) {
            if let Some(id) = selected {
                hierarchy::delete(editor, scene, id);
            }
            ui.close();
        }
    });
}

/// Undo (`back`) or Redo one step. The ask itself touches the frame: a shortcut
/// is consumed before the frame's touch is read, and a write after the step must
/// not fold into an older one (`shell::editor::undo`).
fn step(editor: &mut EditorUi, scene: &mut Scene, back: bool) {
    editor.history_stepped = true;
    let stepped = if back { scene.undo() } else { scene.redo() };
    editor.is_dirty |= stepped.is_some();
}

/// One menu entry: `label` when there is a step, `fallback` greyed out when not.
fn item(
    ui: &mut egui::Ui,
    glyph: &str,
    label: Option<String>,
    fallback: &str,
    shortcut: &KeyboardShortcut,
    playing: bool,
) -> bool {
    let enabled = label.is_some() && !playing;
    let text = format!("{glyph}  {}", label.as_deref().unwrap_or(fallback));
    let button = egui::Button::new(text).shortcut_text(ui.ctx().format_shortcut(shortcut));
    ui.add_enabled(enabled, button).clicked()
}

#[cfg(test)]
#[path = "edit_menu_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "delete_tests.rs"]
mod delete_tests;
