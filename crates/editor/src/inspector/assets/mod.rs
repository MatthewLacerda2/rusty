pub mod audio;
pub mod image;
mod lut;
mod metadata;
pub mod model;
mod nav_areas;
pub mod prefab;
pub mod scene;
pub mod script;
pub mod shader;

use super::preview;
use crate::EditorUi;
use egui_phosphor::regular as icon;
use rusty_asset::{IMAGE_EXTENSIONS, LUT_EXTENSIONS, MODEL_EXTENSIONS};
use rusty_navigation::NavigationGraph;
use rusty_scene::{Scene, PREFAB_EXTENSION, SCENE_EXTENSION};
use rusty_script::scripting::ConsoleLogs;

pub fn draw_inspector(
    ui: &mut egui::Ui,
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    nav: &mut NavigationGraph,
    path: &str,
) {
    let extension = rusty_core::paths::extension(path);

    // Model/texture/shader assets get a Preview tab (#352); everything else (audio,
    // scenes, prefabs, scripts) keeps its single Details view — there's no isolated
    // scene rendering to preview for those. The dispatch table is shared with
    // `Debug.Preview` (#353) so both front-ends accept exactly the same asset kinds.
    if let Some(subject) = rusty_preview::subject_for_path(path) {
        preview::draw_tab_strip(ui, &mut editor.preview_tab_active, editor.theme);
        if editor.preview_tab_active {
            preview::draw(ui, editor, subject);
            return;
        }
    }

    match extension.as_str() {
        e if IMAGE_EXTENSIONS.contains(&e) => {
            image::draw(ui, editor, scene, path);
        }
        // After images: a strip `.png` keeps the image card (#1441).
        e if LUT_EXTENSIONS.contains(&e) => {
            lut::draw(ui, path);
        }
        "wav" | "mp3" | "ogg" => {
            audio::draw(ui, path);
        }
        SCENE_EXTENSION => {
            scene::draw(ui, editor, scene, console, nav, path);
        }
        PREFAB_EXTENSION => {
            prefab::draw(ui, editor, scene, path);
        }
        e if MODEL_EXTENSIONS.contains(&e) => {
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
            ui.heading(format!("{}  File: {}", icon::FILE, filename));
            ui.add_space(5.0);
            ui.label(format!("Path: {}", path));
            ui.colored_label(
                crate::theme::from_ui(ui).text_secondary,
                "Unsupported file extension for specialized inspection.",
            );
        }
    }
}
