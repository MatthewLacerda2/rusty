//! Unreal-style content browser: a folder tree (Sources) on the left and a
//! thumbnail tile grid on the right, with a breadcrumb and a name filter. Lives in
//! its own module so `bottom_panel.rs` stays the panel shell.

use std::path::Path;

use egui_phosphor::regular as icon;

use crate::editor::content_grid;
use crate::editor::EditorUi;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

pub(crate) const ROOT: &str = "project";

/// Entry point: breadcrumb + search, then the split tree / grid view.
pub fn draw(
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    ui: &mut egui::Ui,
) {
    draw_breadcrumb_and_search(editor, ui);
    ui.add_space(editor.theme.space_xs);
    ui.separator();

    ui.horizontal_top(|ui| {
        // Left: folder tree (Sources).
        ui.vertical(|ui| {
            ui.set_width(150.0);
            egui::ScrollArea::vertical()
                .id_source("cb_sources")
                .max_height(140.0)
                .show(ui, |ui| draw_folder_node(editor, ui, ROOT.to_string()));
        });
        ui.separator();
        // Right: asset tile grid.
        ui.vertical(|ui| {
            egui::ScrollArea::vertical()
                .id_source("cb_grid")
                .max_height(140.0)
                .show(ui, |ui| content_grid::draw(editor, scene, console, ui));
        });
    });
}

fn draw_breadcrumb_and_search(editor: &mut EditorUi, ui: &mut egui::Ui) {
    let mut new_dir = None;
    ui.horizontal(|ui| {
        let parts: Vec<&str> = editor
            .current_dir
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();
        let mut acc = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                ui.weak(">");
            }
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(part);
            if ui
                .selectable_label(editor.current_dir == acc, *part)
                .clicked()
            {
                new_dir = Some(acc.clone());
            }
        }
        // Search bar, right-aligned.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut editor.asset_search)
                    .desired_width(140.0)
                    .hint_text(format!("{} search", icon::MAGNIFYING_GLASS)),
            );
        });
    });
    if let Some(dir) = new_dir {
        editor.current_dir = dir;
    }
}

/// Recursive folder tree: only directories, collapsible, clicking selects the
/// folder shown in the grid.
fn draw_folder_node(editor: &mut EditorUi, ui: &mut egui::Ui, path: String) {
    let name = Path::new(&path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&path)
        .to_string();
    let subdirs = read_subdirs(&path);
    let is_current = editor.current_dir == path;

    let header = |ui: &mut egui::Ui| {
        let t = editor.theme;
        ui.colored_label(t.text_secondary, icon::FOLDER);
        if ui.selectable_label(is_current, name).clicked() {
            editor.current_dir = path.clone();
        }
    };

    if subdirs.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().icon_width);
            header(ui);
        });
    } else {
        let id = egui::Id::new(("cb_folder", &path));
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
            .show_header(ui, header)
            .body(|ui| {
                for sub in subdirs {
                    draw_folder_node(editor, ui, sub);
                }
            });
    }
}

fn read_subdirs(path: &str) -> Vec<String> {
    let mut dirs: Vec<String> = std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    dirs.sort_by_key(|p| file_name(p));
    dirs
}

pub(crate) fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

pub(crate) fn extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
}
