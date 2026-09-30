//! src/editor/build_settings.rs — the File → Build Settings window (#431).
//!
//! Edits a draft of the project's [`BuildSettings`] (startup scene, product name,
//! window mode) and writes it to the tracked `project/build_settings.json` on Save.
//! The API twin is the `Application` namespace's setters — one operation, two callers.

use egui_phosphor::regular as icon;

use crate::core::application::{Application, BuildSettings, WindowMode, BUILD_SETTINGS_PATH};
use crate::editor::EditorUi;
use crate::scripting::ConsoleLogs;

/// Draw the window while `editor.show_build_settings` is set. The draft is taken from
/// the live settings when the window opens and dropped when it closes.
pub fn draw(
    editor: &mut EditorUi,
    ctx: &egui::Context,
    app: &mut Application,
    console: &mut ConsoleLogs,
) {
    if !editor.show_build_settings {
        editor.build_settings_draft = None;
        return;
    }
    let current_scene = editor.current_scene_path.clone();
    let draft = editor
        .build_settings_draft
        .get_or_insert_with(|| app.build().clone());
    let mut open = true;
    let mut save = false;
    egui::Window::new(format!("{}  Build Settings", icon::PACKAGE))
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| {
            fields(ui, draft, current_scene.as_deref());
            ui.separator();
            save = ui.button(format!("{}  Save", icon::FLOPPY_DISK)).clicked();
        });
    if save {
        match app.set_build(draft.clone()) {
            Ok(()) => console.info(format!("Build settings saved to {BUILD_SETTINGS_PATH}")),
            Err(err) => console.error(err),
        }
    }
    editor.show_build_settings = open;
}

/// The editable fields: startup scene (typed, or taken from the open scene), product
/// name, and the player's first-launch window mode.
fn fields(ui: &mut egui::Ui, draft: &mut BuildSettings, current_scene: Option<&str>) {
    egui::Grid::new("build_settings_grid")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Startup scene");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut draft.startup_scene);
                let use_current =
                    ui.add_enabled(current_scene.is_some(), egui::Button::new("Use open scene"));
                if let (true, Some(path)) = (use_current.clicked(), current_scene) {
                    draft.startup_scene = path.to_string();
                }
            });
            ui.end_row();

            ui.label("Product name");
            ui.text_edit_singleline(&mut draft.product_name);
            ui.end_row();

            ui.label("Window mode");
            ui.horizontal(|ui| {
                for mode in [WindowMode::Windowed, WindowMode::Fullscreen] {
                    ui.selectable_value(&mut draft.window_mode, mode, mode.name());
                }
            });
            ui.end_row();
        });
}
