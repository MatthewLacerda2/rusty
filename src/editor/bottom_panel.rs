use egui_phosphor::regular as icon;

use crate::editor::{content_browser, EditorUi};
use crate::scene::Scene;
use crate::scripting::{ConsoleLogs, LogLevel};

/// BOTTOM PANEL: Folder Explorer & Console Logs
pub fn draw(
    editor: &mut EditorUi,
    ctx: &egui::Context,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
) {
    let t = editor.theme;
    if !editor.bottom_open {
        draw_collapsed(ctx, t, &mut editor.bottom_open);
        return;
    }
    egui::TopBottomPanel::bottom("Bottom Panel")
        .resizable(true)
        .min_height(160.0)
        .frame(
            egui::Frame::none()
                .fill(t.bg_tier1)
                .inner_margin(t.space_sm)
                .stroke(egui::Stroke::new(1.0, t.border)),
        )
        .show(ctx, |ui| {
            // Tab Header Bar
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(
                        editor.active_bottom_tab == "assets",
                        format!("{}  Content", icon::FOLDERS),
                    )
                    .clicked()
                {
                    editor.active_bottom_tab = "assets".to_string();
                }
                if ui
                    .selectable_label(
                        editor.active_bottom_tab == "console",
                        format!("{}  Console", icon::TERMINAL_WINDOW),
                    )
                    .clicked()
                {
                    editor.active_bottom_tab = "console".to_string();
                }

                // Align the collapse caret and dynamic tab utility button on the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(icon::CARET_DOWN)
                        .on_hover_text("Collapse")
                        .clicked()
                    {
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
            ui.separator();
            ui.add_space(3.0);

            if editor.active_bottom_tab == "assets" {
                content_browser::draw(editor, scene, console, ui);
            } else if editor.active_bottom_tab == "console" {
                draw_console(console, ui);
                #[cfg(feature = "dev")]
                draw_repl_input(editor, ui);
            }
        });
}

/// Collapsed state: a short rail with a caret that reopens the bottom panel.
fn draw_collapsed(ctx: &egui::Context, t: crate::editor::theme::Theme, open: &mut bool) {
    egui::TopBottomPanel::bottom("Bottom Rail")
        .resizable(false)
        .exact_height(28.0)
        .frame(
            egui::Frame::none()
                .fill(t.bg_tier1)
                .inner_margin(t.space_sm)
                .stroke(egui::Stroke::new(1.0, t.border)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button(icon::CARET_UP).on_hover_text("Expand").clicked() {
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
    egui::Window::new(format!("{}  Developer Console", icon::TERMINAL_WINDOW))
        .default_height(220.0)
        .default_width(520.0)
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(8.0, -8.0))
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
        ui.label("lua>");
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
        .id_source("ConsoleScroll")
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
                        LogLevel::Warning => t.accent_yellow,
                        LogLevel::Error => t.danger,
                    };
                    ui.colored_label(color, format!("  {}", msg));
                }
            }
        });
}
