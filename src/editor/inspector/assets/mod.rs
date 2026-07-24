pub mod audio;
pub mod image;
pub mod model;
pub mod prefab;
pub mod scene;
pub mod script;
pub mod shader;

use super::preview;
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

pub fn draw_inspector(
    ui: &mut egui::Ui,
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    nav: &mut NavigationGraph,
    path: &str,
) {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Model/texture/shader assets get a Preview tab (#352); everything else (audio,
    // scenes, prefabs, scripts) keeps its single Details view — there's no isolated
    // scene rendering to preview for those. The dispatch table is shared with
    // `Debug.Preview` (#353) so both front-ends accept exactly the same asset kinds.
    if let Some(subject) = crate::preview::subject_for_path(path) {
        preview::draw_tab_strip(ui, &mut editor.preview_tab_active, editor.theme);
        if editor.preview_tab_active {
            preview::draw(ui, editor, subject);
            return;
        }
    }

    match extension.as_str() {
        "png" | "tga" | "jpg" | "jpeg" => {
            image::draw(ui, editor, scene, path);
        }
        "wav" | "mp3" | "ogg" => {
            audio::draw(ui, editor, path);
        }
        "scene" => {
            scene::draw(ui, editor, scene, console, nav, path);
        }
        "prefab" => {
            prefab::draw(ui, editor, scene, path);
        }
        "fbx" | "obj" | "gltf" | "glb" => {
            model::draw(ui, editor, scene, path);
        }
        "lua" => {
            script::draw(ui, editor, scene, console, path);
        }
        "wgsl" => {
            shader::draw(ui, path);
        }
        _ => {
            let filename = std::path::Path::new(path)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(path);
            ui.heading(format!("📝 File: {}", filename));
            ui.add_space(5.0);
            ui.label(format!("Path: {}", path));
            ui.colored_label(
                egui::Color32::GRAY,
                "Unsupported file extension for specialized inspection.",
            );
        }
    }
}
