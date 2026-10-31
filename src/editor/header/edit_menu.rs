//! The Edit menu and its shortcuts (#1631): Undo / Redo over the scene's history
//! (`scene::undo`), Unity's Edit ▸ Undo and Ctrl+Z / Ctrl+Shift+Z (Cmd on macOS).

use egui::{Key, KeyboardShortcut, Modifiers};
use egui_phosphor::regular as icon;

use crate::editor::EditorUi;
use crate::scene::Scene;

const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
/// Ctrl+Y, Unity's Redo on Windows and Linux.
const REDO_Y: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);

/// Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y, in edit mode, unless a text field has the
/// keyboard: there they edit the text, as in Unity.
pub(super) fn shortcuts(editor: &mut EditorUi, ctx: &egui::Context, scene: &mut Scene) {
    if scene.is_playing() || ctx.egui_wants_keyboard_input() {
        return;
    }
    // Redo first: its shortcut is Undo's plus Shift, and egui matches Undo's
    // logically, Shift or not.
    let redo = ctx.input_mut(|i| i.consume_shortcut(&REDO) || i.consume_shortcut(&REDO_Y));
    if redo {
        editor.is_dirty |= scene.redo().is_some();
    } else if ctx.input_mut(|i| i.consume_shortcut(&UNDO)) {
        editor.is_dirty |= scene.undo().is_some();
    }
}

/// The Edit menu: Undo and Redo, each naming its step; greyed out in Play and
/// when there is nothing to step.
pub(super) fn edit_menu(editor: &mut EditorUi, ui: &mut egui::Ui, scene: &mut Scene) {
    let playing = scene.is_playing();
    ui.menu_button("Edit", |ui| {
        let ctx = ui.ctx().clone();
        let undo = scene.undo_name().map(|n| format!("Undo {n}"));
        if item(
            ui,
            &ctx,
            icon::ARROW_U_UP_LEFT,
            undo,
            "Undo",
            &UNDO,
            playing,
        ) {
            editor.is_dirty |= scene.undo().is_some();
            ui.close();
        }
        let redo = scene.redo_name().map(|n| format!("Redo {n}"));
        if item(
            ui,
            &ctx,
            icon::ARROW_U_UP_RIGHT,
            redo,
            "Redo",
            &REDO,
            playing,
        ) {
            editor.is_dirty |= scene.redo().is_some();
            ui.close();
        }
    });
}

/// One menu entry: `label` when there is a step, `fallback` greyed out when not.
fn item(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    glyph: &str,
    label: Option<String>,
    fallback: &str,
    shortcut: &KeyboardShortcut,
    playing: bool,
) -> bool {
    let enabled = label.is_some() && !playing;
    let text = format!("{glyph}  {}", label.as_deref().unwrap_or(fallback));
    let button = egui::Button::new(text).shortcut_text(ctx.format_shortcut(shortcut));
    ui.add_enabled(enabled, button).clicked()
}
