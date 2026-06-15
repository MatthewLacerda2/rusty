use crate::editor::EditorUi;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;
use std::fs;
use std::path::Path;

pub fn draw(
    ui: &mut egui::Ui,
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    path: &str,
) {
    let filename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);

    ui.heading(format!("🎬 Scene: {}", filename));
    ui.add_space(5.0);

    // File metadata card
    let size_str = if let Ok(meta) = fs::metadata(path) {
        format_size(meta.len())
    } else {
        "Unknown size".to_string()
    };

    // Attempt to inspect JSON content
    let mut entity_count = 0;
    let mut skybox_str = "None".to_string();
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(arr) = val.get("entities").and_then(|e| e.as_array()) {
                entity_count = arr.len();
            }
            if let Some(skybox) = val.get("skybox_path").and_then(|s| s.as_str()) {
                skybox_str = skybox.to_string();
            }
        }
    }

    egui::Frame::none()
        .fill(crate::editor::theme::from_ui(ui).bg_tier2)
        .inner_margin(8.0)
        .rounding(6.0)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(format!("Path: {}", path));
                ui.label(format!("Size: {}", size_str));
                ui.label(format!("Entities serialized: {}", entity_count));
                ui.label(format!("Skybox: {}", skybox_str));
            });
        });
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(5.0);

    // Actions
    ui.heading("Scene Operations");
    ui.add_space(8.0);

    // Button to load scene
    if ui
        .add(egui::Button::new("📂 Load Scene").min_size(egui::Vec2::new(120.0, 30.0)))
        .clicked()
    {
        match scene.load_from_file(path) {
            Ok(_) => {
                editor.current_scene_path = Some(path.to_string());
                editor.selected_entity_id = None;
                editor.selected_asset_path = None;
                editor.is_dirty = true;
                console.info(format!("Successfully loaded scene from {}", filename));
            }
            Err(e) => {
                console.error(format!("Failed to load scene: {}", e));
            }
        }
    }

    ui.add_space(8.0);

    // Button to save/overwrite scene
    if ui
        .add(egui::Button::new("💾 Overwrite with Current").min_size(egui::Vec2::new(120.0, 30.0)))
        .clicked()
    {
        match scene.save_to_file(path) {
            Ok(_) => {
                console.info(format!("Overwrote scene file: {}", filename));
            }
            Err(e) => {
                console.error(format!("Failed to save scene: {}", e));
            }
        }
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
