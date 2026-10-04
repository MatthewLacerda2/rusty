//! The toolbar row under the menu bar: the open scene's name on the left and the
//! Play / Stop transport centred, as in Unity. Play mode is shown by the lit Play
//! button and the editor-wide tint ([`crate::editor::theme::Theme::for_play_mode`]),
//! not by a mode label.

use egui::{Button, CornerRadius, Rect, RichText, Ui, UiBuilder};
use egui_phosphor::regular as icon;

use crate::editor::theme::Theme;
use crate::editor::EditorUi;
use crate::scene::Scene;

/// Ctrl+P (Cmd+P on macOS) toggles Play, as in Unity (#576). Esc does not stop Play:
/// in the Game view it frees the cursor and reaches the game.
const PLAY_SHORTCUT: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::P);

const ROW_HEIGHT: f32 = 26.0;
const BUTTON: egui::Vec2 = egui::vec2(34.0, 22.0);

/// Draw the toolbar row and apply the transport's clicks and shortcut.
pub(super) fn draw(editor: &mut EditorUi, ui: &mut Ui, scene: &mut Scene, is_playing: &mut bool) {
    let t = editor.theme;
    let row = ui.available_rect_before_wrap();
    let row = Rect::from_min_size(row.min, egui::vec2(row.width(), ROW_HEIGHT));
    ui.allocate_rect(row, egui::Sense::hover());

    let layout = egui::Layout::left_to_right(egui::Align::Center);
    let mut left = ui.new_child(UiBuilder::new().max_rect(row).layout(layout));
    left.label(RichText::new(icon::FILM_SLATE).color(t.text_secondary));
    left.label(RichText::new(scene_name(editor)).color(t.text_secondary));

    let group = Rect::from_center_size(row.center(), egui::vec2(BUTTON.x * 2.0 + 2.0, BUTTON.y));
    let mut center = ui.new_child(UiBuilder::new().max_rect(group).layout(layout));
    center.spacing_mut().item_spacing.x = 2.0;
    transport(editor, &mut center, scene, is_playing, &t);
}

/// The Play / Stop pair. Play lights up in the accent while the game runs; the
/// shortcut toggles. Clicking Stop always leaves Play and drops the selection.
fn transport(
    editor: &mut EditorUi,
    ui: &mut Ui,
    scene: &mut Scene,
    is_playing: &mut bool,
    t: &Theme,
) {
    let toggled = ui.input_mut(|i| i.consume_shortcut(&PLAY_SHORTCUT));
    let shortcut = ui.ctx().format_shortcut(&PLAY_SHORTCUT);
    let (play_fill, play_text) = if *is_playing {
        (t.accent, egui::Color32::WHITE)
    } else {
        (t.bg_hover, t.text_primary)
    };
    let play = Button::new(RichText::new(icon::PLAY).size(14.0).color(play_text))
        .fill(play_fill)
        .corner_radius(CornerRadius {
            nw: 4,
            sw: 4,
            ne: 0,
            se: 0,
        })
        .min_size(BUTTON);
    let play = ui.add(play).on_hover_text(format!("Play ({shortcut})"));
    if play.clicked() || (toggled && !*is_playing) {
        *is_playing = true;
    } else if toggled {
        stop(editor, scene, is_playing);
    }

    let stop_text = if *is_playing {
        t.text_primary
    } else {
        t.text_secondary
    };
    let stop_btn = Button::new(RichText::new(icon::STOP).size(14.0).color(stop_text))
        .fill(t.bg_hover)
        .corner_radius(CornerRadius {
            nw: 0,
            sw: 0,
            ne: 4,
            se: 4,
        })
        .min_size(BUTTON);
    if ui
        .add(stop_btn)
        .on_hover_text(format!("Stop ({shortcut})"))
        .clicked()
    {
        stop(editor, scene, is_playing);
    }
}

/// The open scene's file stem, or "Untitled" for a scene never saved.
fn scene_name(editor: &EditorUi) -> String {
    editor
        .current_scene_path
        .as_deref()
        .and_then(|p| std::path::Path::new(p).file_stem())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".to_string())
}

/// Leave Play, dropping a selection that names an entity of the play copy.
fn stop(editor: &mut EditorUi, scene: &mut Scene, is_playing: &mut bool) {
    *is_playing = false;
    scene.selected_entity_id = None;
    editor.selected_entity_id = None;
}
