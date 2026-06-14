use std::path::Path;

use crate::editor::EditorUi;
use crate::scripting::{ConsoleLogs, LogLevel};

/// BOTTOM PANEL: Folder Explorer & Console Logs
pub fn draw(editor: &mut EditorUi, ctx: &egui::Context, console: &mut ConsoleLogs) {
    egui::TopBottomPanel::bottom("Bottom Panel")
        .min_height(160.0)
        .frame(
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(14, 14, 20))
                .inner_margin(8.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(30, 30, 40))),
        )
        .show(ctx, |ui| {
            // Tab Header Bar
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(editor.active_bottom_tab == "assets", "📁 Project Assets")
                    .clicked()
                {
                    editor.active_bottom_tab = "assets".to_string();
                }
                if ui
                    .selectable_label(
                        editor.active_bottom_tab == "console",
                        "📟 Developer Console",
                    )
                    .clicked()
                {
                    editor.active_bottom_tab = "console".to_string();
                }

                // Align dynamic tab utility button on the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if editor.active_bottom_tab == "console" {
                        if ui.button("🗑 Clear Console").clicked() {
                            console.messages.clear();
                        }
                    } else if ui.button("⟲ Root").clicked() {
                        editor.current_dir = "project".to_string();
                    }
                });
            });
            ui.separator();
            ui.add_space(3.0);

            if editor.active_bottom_tab == "assets" {
                draw_assets(editor, ui);
            } else if editor.active_bottom_tab == "console" {
                draw_console(console, ui);
            }
        });
}

fn draw_assets(editor: &mut EditorUi, ui: &mut egui::Ui) {
    // Draw Breadcrumb Bar
    let mut new_dir = None;
    ui.horizontal(|ui| {
        let parts: Vec<&str> = editor
            .current_dir
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();
        let mut path_acc = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                ui.label(">");
            }
            if !path_acc.is_empty() {
                path_acc.push('/');
            }
            path_acc.push_str(part);

            let current_path = path_acc.clone();
            if ui
                .selectable_label(editor.current_dir == current_path, *part)
                .clicked()
            {
                new_dir = Some(current_path);
            }
        }
    });
    if let Some(dir) = new_dir {
        editor.current_dir = dir;
    }
    ui.add_space(5.0);

    egui::ScrollArea::vertical()
        .id_source("FolderExplorerScroll")
        .max_height(120.0)
        .show(ui, |ui| {
            if let Ok(entries) = std::fs::read_dir(&editor.current_dir) {
                let mut entries_list = Vec::new();

                for entry in entries.flatten() {
                    let path = entry.path();
                    let path_str = path.to_string_lossy().to_string().replace("\\", "/");
                    let is_dir = path.is_dir();
                    entries_list.push((path_str, is_dir));
                }

                // Sort entries alphabetically by file/folder name
                entries_list.sort_by(|a, b| {
                    let name_a = Path::new(&a.0)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let name_b = Path::new(&b.0)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    name_a.cmp(&name_b)
                });

                // Render parent directory ".." option if not at the root
                if editor.current_dir != "project" {
                    if let Some(parent) = Path::new(&editor.current_dir).parent() {
                        let parent_str = parent.to_string_lossy().to_string().replace("\\", "/");
                        ui.horizontal(|ui| {
                            if ui.selectable_label(false, "📁 .. (Go Up)").clicked() {
                                editor.current_dir = parent_str;
                            }
                        });
                    }
                }

                for (path_str, is_dir) in entries_list {
                    let filename = Path::new(&path_str)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("File")
                        .to_string();
                    if is_dir {
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(false, format!("📁 {}", filename))
                                .clicked()
                            {
                                editor.current_dir = path_str;
                            }
                        });
                    } else {
                        let extension = Path::new(&path_str)
                            .extension()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        let icon = match extension.as_str() {
                            "png" | "tga" | "jpg" | "jpeg" => "🖼️",
                            "wav" | "mp3" | "ogg" => "🎵",
                            "scene" => "🎬",
                            "fbx" | "obj" => "📐",
                            "lua" => "📄",
                            _ => "📝",
                        };

                        let is_selected = editor.selected_asset_path.as_ref() == Some(&path_str);

                        ui.horizontal(|ui| {
                            let label_res =
                                ui.selectable_label(is_selected, format!("{} {}", icon, filename));
                            if label_res.clicked() {
                                editor.selected_asset_path = Some(path_str.clone());
                                editor.selected_entity_id = None; // Deselect scene entity!

                                // Load initial data for script editing
                                if extension == "lua" {
                                    if let Ok(content) = std::fs::read_to_string(&path_str) {
                                        editor.asset_script_content = content;
                                    }
                                }
                            }
                        });
                    }
                }
            } else {
                ui.colored_label(egui::Color32::RED, "Failed to read directory.");
            }
        });
}

fn draw_console(console: &mut ConsoleLogs, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .id_source("ConsoleScroll")
        .max_height(120.0)
        .show(ui, |ui| {
            if console.messages.is_empty() {
                ui.colored_label(
                    egui::Color32::from_rgb(100, 100, 130),
                    "  No execution logs yet. Logs will print when running.",
                );
            } else {
                for (msg, level) in &console.messages {
                    let color = match level {
                        LogLevel::Info => egui::Color32::from_rgb(220, 220, 230),
                        LogLevel::Warning => egui::Color32::from_rgb(255, 200, 50),
                        LogLevel::Error => egui::Color32::from_rgb(255, 60, 100),
                    };
                    ui.colored_label(color, format!("  {}", msg));
                }
            }
        });
}
