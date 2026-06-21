//! The content browser's right-hand tile grid: asset/folder tiles, the name
//! filter, and the click actions (navigate / select / load scene). Split from
//! `content_browser.rs` to stay under the size gate.

use std::path::Path;

use egui_phosphor::regular as icon;

use crate::editor::content_browser::{extension, file_name, ROOT};
use crate::editor::theme::Theme;
use crate::editor::EditorUi;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// A click on a content tile, resolved after the grid is drawn (so the borrow on
/// `editor` is released before the action mutates it).
enum TileAction {
    Navigate(String),
    Load(String, String),
    Select(String),
}

/// Render the tile grid for the current folder and apply any click action.
pub fn draw(
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    ui: &mut egui::Ui,
) {
    let dir = editor.current_dir.clone();
    let Some(entries) = read_entries(&dir) else {
        ui.colored_label(editor.theme.danger, "Failed to read directory.");
        return;
    };

    let search = editor.asset_search.to_lowercase();
    let t = editor.theme;
    let selected = editor.selected_asset_path.clone();

    let action = ui
        .horizontal_wrapped(|ui| draw_grid(ui, t, &dir, &entries, &search, selected.as_deref()))
        .inner;

    match action {
        Some(TileAction::Navigate(d)) => editor.current_dir = d,
        Some(TileAction::Load(p, n)) => load_scene(editor, scene, console, &p, &n),
        Some(TileAction::Select(p)) => select_asset(editor, p),
        None => {}
    }
}

/// Read the folder into `(path, is_dir)` pairs sorted folders-first, then
/// alphabetically. Returns `None` if the directory can't be read.
fn read_entries(dir: &str) -> Option<Vec<(String, bool)>> {
    let read = std::fs::read_dir(dir).ok()?;
    let mut entries: Vec<(String, bool)> = read
        .flatten()
        .map(|e| {
            let p = e.path();
            (p.to_string_lossy().replace('\\', "/"), p.is_dir())
        })
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then(file_name(&a.0).cmp(&file_name(&b.0))));
    Some(entries)
}

/// Draw the up-tile (when not at root) plus a tile per visible entry, returning
/// the first click action triggered this frame (if any).
fn draw_grid(
    ui: &mut egui::Ui,
    t: Theme,
    dir: &str,
    entries: &[(String, bool)],
    search: &str,
    selected: Option<&str>,
) -> Option<TileAction> {
    let mut action = None;
    if dir != ROOT {
        if let Some(parent) = Path::new(dir).parent() {
            let up = tile(
                ui,
                t,
                "..",
                icon::ARROW_BEND_LEFT_UP,
                t.text_secondary,
                false,
            );
            if up.clicked() {
                action = Some(TileAction::Navigate(
                    parent.to_string_lossy().replace('\\', "/"),
                ));
            }
        }
    }
    for (path, is_dir) in entries {
        let name = file_name(path);
        if !search.is_empty() && !name.to_lowercase().contains(search) {
            continue;
        }
        let ext = extension(path);
        let glyph = if *is_dir {
            icon::FOLDER
        } else {
            type_glyph(&ext)
        };
        let color = if *is_dir {
            t.text_secondary
        } else {
            t.asset_color(&ext)
        };
        let is_sel = selected == Some(path.as_str());
        let resp = tile(ui, t, &name, glyph, color, is_sel);
        if *is_dir {
            if resp.clicked() {
                action = Some(TileAction::Navigate(path.clone()));
            }
        } else if ext == "scene" && resp.double_clicked() {
            action = Some(TileAction::Load(path.clone(), name.clone()));
        } else if resp.clicked() {
            action = Some(TileAction::Select(path.clone()));
        }
    }
    action
}

/// A single asset/folder tile: a type glyph over a (truncated) name. Returns a
/// click-sensing response over the whole tile.
fn tile(
    ui: &mut egui::Ui,
    t: Theme,
    name: &str,
    glyph: &str,
    color: egui::Color32,
    sel: bool,
) -> egui::Response {
    let fill = if sel { t.selection } else { t.bg_tier2 };
    let edge = if sel { t.accent_purple } else { t.border };
    let inner = egui::Frame::none()
        .fill(fill)
        .rounding(4.0)
        .inner_margin(t.space_xs)
        .stroke(egui::Stroke::new(1.0, edge))
        .show(ui, |ui| {
            ui.set_min_size(egui::vec2(72.0, 60.0));
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(glyph).size(26.0).color(color));
                ui.label(egui::RichText::new(truncate(name)).small());
            });
        });
    inner.response.interact(egui::Sense::click())
}

fn type_glyph(ext: &str) -> &'static str {
    match ext {
        "png" | "tga" | "jpg" | "jpeg" => icon::IMAGE,
        "wav" | "mp3" | "ogg" => icon::MUSIC_NOTES,
        "scene" => icon::FILM_SLATE,
        "prefab" => icon::PACKAGE,
        "fbx" | "obj" | "gltf" | "glb" => icon::CUBE,
        "lua" => icon::FILE_CODE,
        _ => icon::FILE,
    }
}

fn truncate(name: &str) -> String {
    if name.chars().count() > 11 {
        format!("{}…", name.chars().take(10).collect::<String>())
    } else {
        name.to_string()
    }
}

/// Select an asset (clears entity selection); loads script text for `.lua`.
fn select_asset(editor: &mut EditorUi, path: String) {
    editor.selected_entity_id = None;
    if extension(&path) == "lua" {
        if let Ok(content) = std::fs::read_to_string(&path) {
            editor.asset_script_content = content;
        }
    }
    editor.selected_asset_path = Some(path);
}

/// Load a scene file into the live World and focus the inspector on its settings
/// (clearing the selections makes the inspector fall through to Scene Settings).
fn load_scene(
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    path: &str,
    filename: &str,
) {
    match scene.load_from_file(path) {
        Ok(_) => {
            editor.current_scene_path = Some(path.to_string());
            editor.selected_entity_id = None;
            editor.selected_asset_path = None;
            editor.is_dirty = true;
            console.info(format!("Loaded scene from {}", filename));
        }
        Err(e) => console.error(format!("Failed to load scene: {}", e)),
    }
}
