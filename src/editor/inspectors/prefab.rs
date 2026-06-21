//! src/editor/inspectors/prefab.rs — `.prefab` asset inspector (#215).
//!
//! Mirrors the model inspector: a metadata card plus an "Instantiate into Scene"
//! button. Instantiate routes through the shared `scene::prefab` verbs (load +
//! `instantiate_prefab`), so it stamps an independent copy with fresh deterministic
//! ids exactly like `Scene.Instantiate` does for a script.

use std::fs;
use std::path::Path;

use crate::editor::EditorUi;
use crate::scene::Scene;

pub fn draw(ui: &mut egui::Ui, editor: &mut EditorUi, scene: &mut Scene, path: &str) {
    let filename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);

    ui.heading(format!("📦 Prefab: {}", filename));
    ui.add_space(5.0);

    draw_metadata_card(ui, path);

    ui.add_space(15.0);
    ui.separator();
    ui.add_space(5.0);

    ui.heading("Instantiation");
    ui.add_space(5.0);
    ui.colored_label(
        egui::Color32::GRAY,
        "Stamps an independent (unpacked) copy into the scene.",
    );
    ui.add_space(5.0);

    if ui
        .add(egui::Button::new("➕ Instantiate into Scene").min_size(egui::Vec2::new(140.0, 30.0)))
        .clicked()
    {
        instantiate(editor, scene, path);
    }
}

/// File metadata card: path, on-disk size, entity + material counts read from the
/// document (so the agent/user sees the subtree's shape before stamping it).
fn draw_metadata_card(ui: &mut egui::Ui, path: &str) {
    egui::Frame::none()
        .fill(crate::editor::theme::from_ui(ui).bg_tier2)
        .inner_margin(8.0)
        .rounding(6.0)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(format!("Path: {}", path));
                let size_str = match fs::metadata(path) {
                    Ok(meta) => format_size(meta.len()),
                    Err(_) => "Unknown size".to_string(),
                };
                ui.label(format!("Size: {}", size_str));
                if let Some((entities, materials)) = counts(path) {
                    ui.label(format!("Entities: {entities}"));
                    ui.label(format!("Materials: {materials}"));
                }
            });
        });
}

/// Read the prefab document and report its `(entity_count, material_count)`, or
/// `None` if it cannot be read/parsed.
fn counts(path: &str) -> Option<(usize, usize)> {
    let json = fs::read_to_string(path).ok()?;
    let prefab: crate::scene::PrefabData = serde_json::from_str(&json).ok()?;
    Some((prefab.entities.len(), prefab.materials.len()))
}

/// Load + instantiate the prefab into the scene root, select the new root, and mark
/// the editor dirty — the same effect as the model inspector's instantiate.
fn instantiate(editor: &mut EditorUi, scene: &mut Scene, path: &str) {
    if let Ok(root_id) = crate::scene::load_and_instantiate(scene, path, None) {
        editor.is_dirty = true;
        editor.selected_entity_id = Some(root_id);
        editor.selected_asset_path = None;
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
