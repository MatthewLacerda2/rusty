use egui_phosphor::regular as icon;

use crate::editor::theme::chrome;
use crate::editor::{content_browser, EditorUi};
use crate::scene::Scene;
use crate::scripting::{ConsoleLogs, LogLevel};

/// BOTTOM PANEL: Folder Explorer & Console Logs
pub fn draw(
    editor: &mut EditorUi,
    ui: &mut egui::Ui,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
) {
    let t = editor.theme;
    if !editor.bottom_open {
        draw_collapsed(ui, t, &mut editor.bottom_open);
        return;
    }
    let frame = chrome::panel_frame(&t);
    egui::Panel::bottom("Bottom Panel")
        .resizable(true)
        .min_size(112.0 + frame.inner_margin.sum().y)
        .frame(frame)
        .show(ui, |ui| {
            draw_tab_header(editor, console, ui);
            chrome::hairline(ui, &t);

            if editor.active_bottom_tab == "assets" {
                content_browser::draw(editor, scene, console, ui);
            } else if editor.active_bottom_tab == "console" {
                draw_console(console, ui);
                #[cfg(feature = "dev")]
                draw_repl_input(editor, ui);
            }
        });
}

/// The tab header bar: the Content/Console selectors plus the right-aligned
/// collapse caret and the per-tab utility button (Clear logs / jump to Root).
fn draw_tab_header(editor: &mut EditorUi, console: &mut ConsoleLogs, ui: &mut egui::Ui) {
    let t = editor.theme;
    ui.horizontal(|ui| {
        let tabs = [
            ("assets", format!("{}  Content", icon::FOLDERS)),
            ("console", format!("{}  Console", icon::TERMINAL_WINDOW)),
        ];
        for (key, label) in tabs {
            let selected = editor.active_bottom_tab == key;
            if chrome::tab(ui, &t, selected, &label).clicked() {
                editor.active_bottom_tab = key.to_string();
            }
        }

        // Align the collapse caret and dynamic tab utility button on the right
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if chrome::icon_button(ui, icon::CARET_DOWN, "Collapse").clicked() {
                editor.bottom_open = false;
            }
            if editor.active_bottom_tab == "console" {
                if ui.button(format!("{}  Clear", icon::TRASH)).clicked() {
                    console.messages.clear();
                }
            } else if ui.button(format!("{}  Root", icon::HOUSE)).clicked() {
                editor.current_dir = "project".to_string();
            }
        });
    });
}

/// Collapsed state: a short rail with a caret that reopens the bottom panel.
fn draw_collapsed(ui: &mut egui::Ui, t: crate::editor::theme::Theme, open: &mut bool) {
    let frame = chrome::rail_frame(&t);
    egui::Panel::bottom("Bottom Rail")
        .resizable(false)
        .exact_size(28.0 + frame.inner_margin.sum().y)
        .frame(frame)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if chrome::icon_button(ui, icon::CARET_UP, "Expand").clicked() {
                    *open = true;
                }
                ui.colored_label(
                    t.text_secondary,
                    format!("{}  Content / Console", icon::FOLDERS),
                );
            });
        });
}

/// Floating console shown during Play (dev builds only): the log buffer plus the
/// live Lua REPL input line, so you can call the API while the game runs.
#[cfg(feature = "dev")]
pub fn draw_play_console(editor: &mut EditorUi, ctx: &egui::Context, console: &mut ConsoleLogs) {
    // Open bottom-left on first frame, then let the user move/resize it freely:
    // `default_pos` (one-time) replaces an every-frame `anchor` that snapped drags
    // back, and `resizable` + a min size give it handles without letting it collapse.
    let bottom_left = egui::pos2(8.0, ctx.content_rect().bottom() - 228.0);
    egui::Window::new(format!("{}  Developer Console", icon::TERMINAL_WINDOW))
        .resizable(true)
        .min_width(320.0)
        .min_height(140.0)
        .default_width(520.0)
        .default_height(220.0)
        .default_pos(bottom_left)
        .show(ctx, |ui| {
            draw_console(console, ui);
            draw_repl_input(editor, ui);
        });
}

/// The REPL input line. On submit it stashes the text in `editor.pending_repl`;
/// the front-end drains that and runs it through the single `dev::console`
/// evaluator against the live runtime (so windowed and headless can't drift).
#[cfg(feature = "dev")]
fn draw_repl_input(editor: &mut EditorUi, ui: &mut egui::Ui) {
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("lua>").monospace());
        let resp = ui.add(
            egui::TextEdit::singleline(&mut editor.repl_input.buffer)
                .desired_width(f32::INFINITY)
                .hint_text("e.g. print(Transform.GetPosition(Scene.FindEntityByName(\"Player\")))")
                .font(egui::TextStyle::Monospace),
        );
        let submit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if submit {
            if let Some(line) = editor.repl_input.take_submit() {
                editor.pending_repl = Some(line);
            }
            resp.request_focus();
        }
    });
}

fn draw_console(console: &mut ConsoleLogs, ui: &mut egui::Ui) {
    let t = crate::editor::theme::from_ui(ui);
    egui::ScrollArea::vertical()
        .id_salt("ConsoleScroll")
        .max_height(120.0)
        .show(ui, |ui| {
            if console.messages.is_empty() {
                ui.colored_label(
                    t.text_secondary,
                    "  No execution logs yet. Logs will print when running.",
                );
            } else {
                for (msg, level) in &console.messages {
                    let color = match level {
                        LogLevel::Info => t.text_primary,
                        LogLevel::Warning => t.warning,
                        LogLevel::Error => t.danger,
                    };
                    ui.label(egui::RichText::new(msg).monospace().color(color));
                }
            }
        });
}
