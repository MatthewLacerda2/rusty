use egui_phosphor::regular as icon;

use crate::editor::menu_create;
use crate::editor::EditorUi;
use crate::render::postfx::QualityPreset;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// TOP BAR — a classic menu bar (File / GameObject / Config / About) over a
/// minimal transport row (Play / Stop). Always visible. Scene I/O, object
/// creation and quality live in the menus; the transport row is just play-state.
pub fn draw(
    editor: &mut EditorUi,
    ctx: &egui::Context,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    is_playing: &mut bool,
) {
    let t = editor.theme;
    egui::TopBottomPanel::top("Header Panel")
        .frame(
            egui::Frame::none()
                .fill(t.bg_tier2)
                .inner_margin(t.space_sm)
                .stroke(egui::Stroke::new(1.0, t.border)),
        )
        .show(ctx, |ui| {
            draw_menu_bar(editor, ui, scene, console);
            ui.add_space(t.space_xs);
            ui.separator();
            ui.add_space(t.space_xs);
            draw_transport(editor, ui, scene, is_playing);
        });

    draw_about_window(editor, ctx);
}

/// File / GameObject / Config / About menus.
fn draw_menu_bar(
    editor: &mut EditorUi,
    ui: &mut egui::Ui,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
) {
    egui::menu::bar(ui, |ui| {
        file_menu(editor, ui, scene, console);

        // The GameObject menu — the Unity-style home for creating objects (#255).
        menu_create::game_object_menu(editor, ui, scene);

        config_menu(editor, ui);

        ui.menu_button(format!("{}  About", icon::INFO), |ui| {
            if ui.button("About rusty").clicked() {
                editor.show_about = true;
                ui.close_menu();
            }
        });
    });
}

/// The File menu — scene lifecycle (new / reset / load / save).
fn file_menu(
    editor: &mut EditorUi,
    ui: &mut egui::Ui,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
) {
    ui.menu_button(format!("{}  File", icon::FILE), |ui| {
        if ui.button(format!("{}  New Scene", icon::FILE)).clicked() {
            new_scene(editor, scene, console);
            ui.close_menu();
        }
        if ui
            .button(format!("{}  Reset Scene", icon::ARROW_COUNTER_CLOCKWISE))
            .clicked()
        {
            reset_scene(editor, scene, console);
            ui.close_menu();
        }
        if ui
            .button(format!("{}  Load Scene", icon::FOLDER_OPEN))
            .clicked()
        {
            load_scene(editor, scene, console);
            ui.close_menu();
        }
        if ui
            .button(format!("{}  Save Scene", icon::FLOPPY_DISK))
            .clicked()
        {
            save_scene(editor, scene, console);
            ui.close_menu();
        }
    });
}

/// The Config menu — video/quality presets and Scene Settings.
fn config_menu(editor: &mut EditorUi, ui: &mut egui::Ui) {
    ui.menu_button(format!("{}  Config", icon::GEAR), |ui| {
        ui.label("Video / Quality");
        ui.selectable_value(&mut editor.quality_preset, QualityPreset::Low, "Low");
        ui.selectable_value(&mut editor.quality_preset, QualityPreset::Medium, "Medium");
        ui.selectable_value(&mut editor.quality_preset, QualityPreset::High, "High");
        ui.separator();
        if ui
            .button(format!("{}  Scene Settings", icon::GLOBE))
            .clicked()
        {
            // Focus the inspector on the active scene: clearing both selections
            // makes the inspector fall through to its scene-settings view.
            editor.selected_entity_id = None;
            editor.selected_asset_path = None;
            ui.close_menu();
        }
    });
}

/// Minimal transport row: Play / Stop + the current mode label.
fn draw_transport(
    editor: &mut EditorUi,
    ui: &mut egui::Ui,
    scene: &mut Scene,
    is_playing: &mut bool,
) {
    let t = editor.theme;
    ui.horizontal(|ui| {
        let play_btn = if *is_playing {
            egui::Button::new(
                egui::RichText::new(format!("{}  Play", icon::PLAY))
                    .color(egui::Color32::WHITE)
                    .strong(),
            )
            .fill(t.accent_purple)
        } else {
            egui::Button::new(
                egui::RichText::new(format!("{}  Play", icon::PLAY)).color(t.text_secondary),
            )
        };
        if ui.add(play_btn).clicked() {
            *is_playing = true;
        }

        let stop_btn = if !*is_playing {
            egui::Button::new(
                egui::RichText::new(format!("{}  Stop", icon::STOP))
                    .color(egui::Color32::WHITE)
                    .strong(),
            )
            .fill(t.accent_purple)
        } else {
            egui::Button::new(
                egui::RichText::new(format!("{}  Stop", icon::STOP)).color(t.text_secondary),
            )
        };
        if ui.add(stop_btn).clicked() {
            *is_playing = false;
            scene.selected_entity_id = None;
            editor.selected_entity_id = None;
        }

        ui.separator();
        ui.label(format!(
            "Mode: {}",
            if *is_playing {
                "PLAYMODE"
            } else {
                "EDITORMODE"
            }
        ));
    });
}

/// The About modal window. Toggled from the About menu.
fn draw_about_window(editor: &mut EditorUi, ctx: &egui::Context) {
    let mut open = editor.show_about;
    egui::Window::new(format!("{}  About rusty", icon::INFO))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label("rusty — a 3D game engine that copies Unity's runtime model,");
            ui.label("built with agentic coding in mind.");
        });
    editor.show_about = open;
}

/// Save writes back to the CURRENT scene path (set on load / double-click),
/// falling back to the seeded default scene.
fn current_path(editor: &EditorUi) -> String {
    editor
        .current_scene_path
        .clone()
        .unwrap_or_else(|| crate::scene::DEFAULT_SCENE_PATH.to_string())
}

fn save_scene(editor: &mut EditorUi, scene: &Scene, console: &mut ConsoleLogs) {
    let path = current_path(editor);
    match scene.save_to_file(&path) {
        Ok(_) => {
            editor.current_scene_path = Some(path.clone());
            console.info(format!("Scene saved to {}", path));
        }
        Err(err) => console.error(format!("Failed to save scene: {}", err)),
    }
}

fn load_scene(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) {
    let path = current_path(editor);
    match scene.load_from_file(&path) {
        Ok(_) => {
            editor.current_scene_path = Some(path.clone());
            editor.selected_entity_id = None;
            editor.is_dirty = true;
            console.info(format!("Scene loaded from {}", path));
        }
        Err(err) => console.error(format!("Failed to load scene: {}", err)),
    }
}

/// Start a fresh, empty scene — a blank slate (no entities, default ambient/sky/
/// layers), discarding the in-memory scene. Distinct from Reset Scene, which
/// reverts to the *seeded default* (its ground, lights, etc.). The new scene is
/// unsaved, so its path is cleared until the next Save.
fn new_scene(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) {
    *scene = Scene::new();
    editor.current_scene_path = None;
    editor.selected_entity_id = None;
    editor.selected_asset_path = None;
    editor.is_dirty = true;
    console.info("New empty scene".to_string());
}

/// Revert to the seeded default scene (discards the in-memory scene).
fn reset_scene(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) {
    let path = crate::scene::seed_default_scene();
    match scene.load_from_file(&path) {
        Ok(_) => {
            editor.current_scene_path = Some(path.clone());
            editor.selected_entity_id = None;
            editor.selected_asset_path = None;
            editor.is_dirty = true;
            console.info("Scene reset to default".to_string());
        }
        Err(err) => console.error(format!("Failed to reset scene: {}", err)),
    }
}
