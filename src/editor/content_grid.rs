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

/// Render the tile grid for the current folder and apply any click action.
#[allow(clippy::too_many_lines)]
pub fn draw(
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    ui: &mut egui::Ui,
) {
    let dir = editor.current_dir.clone();
    let Ok(read) = std::fs::read_dir(&dir) else {
        ui.colored_label(editor.theme.danger, "Failed to read directory.");
        return;
    };
    let mut entries: Vec<(String, bool)> = read
        .flatten()
        .map(|e| {
            let p = e.path();
            (p.to_string_lossy().replace('\\', "/"), p.is_dir())
        })
        .collect();
    // Folders first, then alphabetical.
    entries.sort_by(|a, b| b.1.cmp(&a.1).then(file_name(&a.0).cmp(&file_name(&b.0))));

    let search = editor.asset_search.to_lowercase();
    let t = editor.theme;
    let selected = editor.selected_asset_path.clone();

    let mut nav_to = None;
    let mut select = None;
    let mut load = None;
    ui.horizontal_wrapped(|ui| {
        if dir != ROOT {
            if let Some(parent) = Path::new(&dir).parent() {
                let up = tile(
                    ui,
                    t,
                    "..",
                    icon::ARROW_BEND_LEFT_UP,
                    t.text_secondary,
                    false,
                );
                if up.clicked() {
                    nav_to = Some(parent.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        for (path, is_dir) in &entries {
            let name = file_name(path);
            if !search.is_empty() && !name.to_lowercase().contains(&search) {
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
            let is_sel = selected.as_deref() == Some(path.as_str());
            let resp = tile(ui, t, &name, glyph, color, is_sel);
            if *is_dir {
                if resp.clicked() {
                    nav_to = Some(path.clone());
                }
            } else if ext == "scene" && resp.double_clicked() {
                load = Some((path.clone(), name.clone()));
            } else if resp.clicked() {
                select = Some(path.clone());
            }
        }
    });

    if let Some(d) = nav_to {
        editor.current_dir = d;
    } else if let Some((p, n)) = load {
        load_scene(editor, scene, console, &p, &n);
    } else if let Some(p) = select {
        select_asset(editor, p);
    }
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
