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

    let action = draw_grid(ui, t, &dir, &entries, &search, selected.as_deref());

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

/// One tile to draw: what it shows and what clicking it does.
struct Tile {
    name: String,
    glyph: &'static str,
    color: egui::Color32,
    selected: bool,
    path: String,
    kind: TileKind,
}

#[derive(Clone, Copy, PartialEq)]
enum TileKind {
    Up,
    Folder,
    Scene,
    File,
}

const TILE_SIZE: egui::Vec2 = egui::vec2(76.0, 64.0);

/// Draw the up-tile (when not at root) plus a tile per visible entry in rows sized
/// to the panel's width — never past its right edge — returning the first click
/// action triggered this frame (if any).
fn draw_grid(
    ui: &mut egui::Ui,
    t: Theme,
    dir: &str,
    entries: &[(String, bool)],
    search: &str,
    selected: Option<&str>,
) -> Option<TileAction> {
    let tiles = collect_tiles(t, dir, entries, search, selected);
    let gap = ui.spacing().item_spacing.x;
    let columns = ((ui.available_width() + gap) / (TILE_SIZE.x + gap)).max(1.0) as usize;
    let mut action = None;
    for row in tiles.chunks(columns) {
        ui.horizontal(|ui| {
            for tile in row {
                let resp = draw_tile(ui, t, tile);
                action = action.take().or_else(|| tile_action(tile, &resp));
            }
        });
    }
    action
}

fn collect_tiles(
    t: Theme,
    dir: &str,
    entries: &[(String, bool)],
    search: &str,
    selected: Option<&str>,
) -> Vec<Tile> {
    let mut tiles = Vec::new();
    if dir != ROOT {
        if let Some(parent) = Path::new(dir).parent() {
            tiles.push(Tile {
                name: "..".to_string(),
                glyph: icon::ARROW_BEND_LEFT_UP,
                color: t.text_secondary,
                selected: false,
                path: parent.to_string_lossy().replace('\\', "/"),
                kind: TileKind::Up,
            });
        }
    }
    for (path, is_dir) in entries {
        let name = file_name(path);
        if !search.is_empty() && !name.to_lowercase().contains(search) {
            continue;
        }
        let ext = extension(path);
        let (glyph, color, kind) = match (*is_dir, ext.as_str()) {
            (true, _) => (
                icon::FOLDER,
                t.warning.linear_multiply(0.8),
                TileKind::Folder,
            ),
            (false, "scene") => (type_glyph(&ext), t.asset_color(&ext), TileKind::Scene),
            (false, _) => (type_glyph(&ext), t.asset_color(&ext), TileKind::File),
        };
        tiles.push(Tile {
            selected: selected == Some(path.as_str()),
            name,
            glyph,
            color,
            path: path.clone(),
            kind,
        });
    }
    tiles
}

/// What a click on `tile` does: folders open on click, scenes load on
/// double-click, every other file (and a scene's single click) selects.
fn tile_action(tile: &Tile, resp: &egui::Response) -> Option<TileAction> {
    match tile.kind {
        TileKind::Up | TileKind::Folder if resp.clicked() => {
            Some(TileAction::Navigate(tile.path.clone()))
        }
        TileKind::Scene if resp.double_clicked() => {
            Some(TileAction::Load(tile.path.clone(), tile.name.clone()))
        }
        TileKind::Scene | TileKind::File if resp.clicked() => {
            Some(TileAction::Select(tile.path.clone()))
        }
        _ => None,
    }
}

/// A single asset/folder tile: a type glyph over a (truncated) name. Returns a
/// click-sensing response over the whole tile. Hover lifts it a step.
fn draw_tile(ui: &mut egui::Ui, t: Theme, tile: &Tile) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(TILE_SIZE, egui::Sense::click());
    let (fill, edge) = if tile.selected {
        (t.selection, t.accent)
    } else if resp.hovered() {
        (t.bg_hover, t.outline)
    } else {
        (t.bg_tier2, egui::Color32::TRANSPARENT)
    };
    let p = ui.painter();
    let stroke = egui::Stroke::new(1.0, edge);
    p.rect(rect, 6.0, fill, stroke, egui::StrokeKind::Middle);
    let center = egui::Align2::CENTER_CENTER;
    let glyph_pos = rect.center_top() + egui::vec2(0.0, 24.0);
    p.text(
        glyph_pos,
        center,
        tile.glyph,
        egui::FontId::proportional(24.0),
        tile.color,
    );
    let name_pos = rect.center_bottom() - egui::vec2(0.0, 12.0);
    let small = egui::TextStyle::Small.resolve(ui.style());
    p.text(
        name_pos,
        center,
        truncate(&tile.name),
        small,
        t.text_primary,
    );
    resp.on_hover_text(&tile.name)
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
