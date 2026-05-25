pub mod image;
pub mod audio;
pub mod scene;
pub mod model;
pub mod script;

use crate::editor::EditorUi;
use crate::core::scene::Scene;
use crate::scripting::ConsoleLogs;

pub fn draw_inspector(ui: &mut egui::Ui, editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs, path: &str) {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
        
    match extension.as_str() {
        "png" | "tga" | "jpg" | "jpeg" => {
            image::draw(ui, editor, scene, path);
        }
        "wav" | "mp3" | "ogg" => {
            audio::draw(ui, editor, path);
        }
        "scene" => {
            scene::draw(ui, editor, scene, console, path);
        }
        "fbx" | "obj" => {
            model::draw(ui, editor, scene, path);
        }
        "lua" => {
            script::draw(ui, editor, scene, console, path);
        }
        _ => {
            let filename = std::path::Path::new(path)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(path);
            ui.heading(format!("📝 File: {}", filename));
            ui.add_space(5.0);
            ui.label(format!("Path: {}", path));
            ui.colored_label(egui::Color32::GRAY, "Unsupported file extension for specialized inspection.");
        }
    }
}
