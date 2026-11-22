//! One Play button that turns into Stop in the same place, so stopping never moves
//! the cursor. Play mode reads from the lit Stop button and the editor-wide tint
//! ([`crate::theme::Theme::for_play_mode`]), not from a mode label.

use egui::{Button, CornerRadius, RichText, Ui};
use egui_phosphor::regular as icon;

use crate::theme::Theme;
use crate::EditorUi;
use rusty_scene::Scene;

const PLAY_SHORTCUT: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::P);

/// Unity's Console shortcut, not a backtick: a game may bind that to its own console.
#[cfg(feature = "dev")]
const CONSOLE_SHORTCUT: egui::KeyboardShortcut = egui::KeyboardShortcut::new(
    egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
    egui::Key::C,
);

const BUTTON: egui::Vec2 = egui::vec2(34.0, 22.0);

pub(super) fn draw(editor: &mut EditorUi, ui: &mut Ui, scene: &mut Scene, is_playing: &mut bool) {
    let t = editor.theme;
    #[cfg(feature = "dev")]
    console_toggle(editor, ui, &t);
    transport(editor, ui, scene, is_playing, &t);
}

fn button(glyph: &str, lit: bool, t: &Theme) -> Button<'static> {
    let (fill, text) = if lit {
        (t.accent, egui::Color32::WHITE)
    } else {
        (t.bg_hover, t.text_primary)
    };
    Button::new(RichText::new(glyph.to_string()).size(14.0).color(text))
        .fill(fill)
        .corner_radius(CornerRadius::same(4))
        .min_size(BUTTON)
}

#[cfg(feature = "dev")]
fn console_toggle(editor: &mut EditorUi, ui: &mut Ui, t: &Theme) {
    if ui.input_mut(|i| i.consume_shortcut(&CONSOLE_SHORTCUT)) {
        editor.dev.show_play_console = !editor.dev.show_play_console;
    }
    let shown = editor.dev.show_play_console;
    let shortcut = ui.ctx().format_shortcut(&CONSOLE_SHORTCUT);
    let label = if shown { "Hide" } else { "Show" };
    if ui
        .add(button(icon::TERMINAL_WINDOW, shown, t))
        .on_hover_text(format!(
            "{label} the Developer Console during Play ({shortcut})"
        ))
        .clicked()
    {
        editor.dev.show_play_console = !shown;
    }
}

fn transport(
    editor: &mut EditorUi,
    ui: &mut Ui,
    scene: &mut Scene,
    is_playing: &mut bool,
    t: &Theme,
) {
    let refusal = (!*is_playing).then(|| play_refusal(scene)).flatten();
    let toggled = ui.input_mut(|i| i.consume_shortcut(&PLAY_SHORTCUT)) && refusal.is_none();
    let shortcut = ui.ctx().format_shortcut(&PLAY_SHORTCUT);
    let (glyph, verb) = if *is_playing {
        (icon::STOP, "Stop")
    } else {
        (icon::PLAY, "Play")
    };
    let clicked = ui
        .add_enabled(refusal.is_none(), button(glyph, *is_playing, t))
        .on_hover_text(format!("{verb} ({shortcut})"))
        .on_disabled_hover_text(refusal.unwrap_or_default())
        .clicked();
    if clicked || toggled {
        if *is_playing {
            stop(editor, scene, is_playing);
        } else {
            *is_playing = true;
        }
    }
}

/// Why Play is refused now: only the dev build
/// bakes, so only it refuses.
fn play_refusal(scene: &Scene) -> Option<String> {
    #[cfg(feature = "dev")]
    return scene.play_refusal();
    // lint: allow(gpu-default-only) ship stub: no play refusal without dev's bakes; a value, nothing drawn
    #[cfg(not(feature = "dev"))]
    {
        let _ = scene;
        None
    }
}

/// Leave Play, dropping a selection that names an entity of the play copy.
fn stop(editor: &mut EditorUi, scene: &mut Scene, is_playing: &mut bool) {
    *is_playing = false;
    scene.selected_entity_id = None;
    editor.selected_entity_id = None;
}

#[cfg(all(test, feature = "dev"))]
#[path = "transport_bake_tests.rs"]
mod bake_tests;
#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
