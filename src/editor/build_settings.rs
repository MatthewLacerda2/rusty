//! src/editor/build_settings.rs — the File → Build Settings window (#431).
//!
//! Edits a draft of the project's [`BuildSettings`] (startup scene, product name,
//! window mode) and writes it to the project's tracked `project.rusty` on Save.
//! The API twin is the `Application` namespace's setters — one operation, two callers.
//!
//! Below them, the project's default player graphics options (#1438): one checkbox
//! per option, each written to `project.rusty` the moment it changes through
//! `Application::set_default_option`, the op `Graphics.SetDefaultOption` calls; the
//! live options follow it where the player hasn't chosen, to preview a fresh install.

use egui_phosphor::regular as icon;

use crate::core::application::{Application, BuildSettings, WindowMode};
use crate::core::project::PROJECT_FILE;
use crate::core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use crate::editor::EditorUi;
use crate::scripting::ConsoleLogs;

/// Draw the window while `editor.show_build_settings` is set. The draft is taken from
/// the live settings when the window opens and dropped when it closes.
pub fn draw(
    editor: &mut EditorUi,
    ctx: &egui::Context,
    app: &mut Application,
    live: &mut GraphicsOptions,
    console: &mut ConsoleLogs,
) {
    if !editor.show_build_settings {
        editor.build_settings_draft = None;
        return;
    }
    let current_scene = editor.current_scene_path();
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
            ui.separator();
            graphics_defaults(ui, app, live, console);
        });
    if save {
        match app.set_build(draft.clone()) {
            Ok(()) => console.info(format!("Build settings saved to {PROJECT_FILE}")),
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

/// The project's default player graphics options: what a fresh install's options
/// menu starts with. Each change is written at once (no draft), like the API's, and
/// reaches the `live` options wherever the player hasn't set them.
fn graphics_defaults(
    ui: &mut egui::Ui,
    app: &mut Application,
    live: &mut GraphicsOptions,
    console: &mut ConsoleLogs,
) {
    ui.label("Player graphics defaults");
    let defaults = app.graphics_defaults();
    for option in GraphicsOption::ALL {
        let mut on = defaults.allows(option);
        if ui.checkbox(&mut on, option.name()).changed() {
            if let Err(err) = app.set_default_option(option, on, live) {
                console.error(err);
            }
        }
    }
}
