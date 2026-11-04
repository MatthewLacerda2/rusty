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
//!
//! Then the project's render-scale default (#1715), a factor or *Match display*,
//! through `Application::set_default_render_scale`, the op
//! `Graphics.SetDefaultRenderScale` calls; it runs at once unless the player chose a
//! scale.
//!
//! Last, the project's audio settings (#1749): the speed of sound doppler resolves
//! with, written through `Application::set_speed_of_sound` (the op
//! `Audio.SetDefaultSpeedOfSound` calls) once a drag or a typed edit ends, and
//! applied to the audio on entering Play.

use egui_phosphor::regular as icon;

use crate::core::application::{Application, BuildSettings, WindowMode};
use crate::core::project::PROJECT_FILE;
use crate::core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use crate::core::settings::render_scale::{RenderScale, RenderScaleDefault, RenderScaleSetting};
use crate::core::settings::GraphicsSettings;
use crate::editor::widgets::DragRange;
use crate::editor::EditorUi;
use crate::scripting::ConsoleLogs;

/// Draw the window while `editor.show_build_settings` is set. The draft is taken from
/// the live settings when the window opens and dropped when it closes.
pub fn draw(
    editor: &mut EditorUi,
    ctx: &egui::Context,
    app: &mut Application,
    live: &mut GraphicsSettings,
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
            graphics_defaults(ui, app, &mut live.options, console);
            ui.separator();
            render_scale_default(ui, app, &mut live.render_scale, console);
            ui.separator();
            audio(ui, app, console);
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

/// The project's render-scale default: a factor, or *Match display* (`1 / display
/// scale`). A drag or a typed factor is held as a draft, as [`audio`]'s is, and
/// written once it ends.
fn render_scale_default(
    ui: &mut egui::Ui,
    app: &mut Application,
    live: &mut RenderScaleSetting,
    console: &mut ConsoleLogs,
) {
    let current = app.render_scale_default();
    let id = ui.id().with("render_scale_default");
    let draft = ui.data(|d| d.get_temp::<f32>(id));
    let mut next = None;
    ui.horizontal(|ui| {
        ui.label("Default render scale");
        let matching = current == RenderScaleDefault::MatchDisplay;
        if ui.selectable_label(matching, "Match display").clicked() && !matching {
            next = Some(RenderScaleDefault::MatchDisplay);
        }
        let mut factor = draft.unwrap_or(match current {
            RenderScaleDefault::Fixed(scale) => scale.get(),
            RenderScaleDefault::MatchDisplay => 1.0,
        });
        let range = RenderScale::MIN..=RenderScale::MAX;
        let drag = egui::DragValue::new(&mut factor).speed(0.01).ranged(range);
        let r = ui.add_enabled(!matching, drag);
        if matching && ui.small_button("Fixed").clicked() {
            next = Some(RenderScaleDefault::Fixed(RenderScale::NATIVE));
        }
        let editing = r.dragged() || r.has_focus();
        if r.changed() && editing {
            ui.data_mut(|d| d.insert_temp(id, factor));
        } else if r.changed() || (!editing && draft.is_some()) {
            ui.data_mut(|d| d.remove::<f32>(id));
            next = Some(RenderScaleDefault::Fixed(RenderScale::new(factor)));
        }
    });
    if live.chosen.is_some() {
        ui.weak("The player's saved render scale overrides it here.");
    }
    if let Some(default) = next.filter(|d| *d != current) {
        if let Err(err) = app.set_default_render_scale(default, live) {
            console.error(err);
        }
    }
}

/// The project's audio settings: the speed of sound (m/s). A drag or a typed edit
/// is held as a draft in egui's memory and written once, when it ends, not on every
/// frame it changes.
fn audio(ui: &mut egui::Ui, app: &mut Application, console: &mut ConsoleLogs) {
    let id = ui.id().with("speed_of_sound");
    let draft = ui.data(|d| d.get_temp::<f32>(id));
    let mut c = draft.unwrap_or(app.audio_settings().speed_of_sound);
    ui.horizontal(|ui| {
        ui.label("Speed of sound (m/s)");
        let drag = egui::DragValue::new(&mut c)
            .speed(1.0)
            .ranged(1.0..=10_000.0);
        let r = ui.add(drag);
        let editing = r.dragged() || r.has_focus();
        if r.changed() && editing {
            ui.data_mut(|d| d.insert_temp(id, c));
        } else if r.changed() || (!editing && draft.is_some()) {
            ui.data_mut(|d| d.remove::<f32>(id));
            if let Err(err) = app.set_speed_of_sound(c) {
                console.error(err);
            }
        }
    });
}
