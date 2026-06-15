use egui_phosphor::regular as icon;

use crate::editor::EditorUi;
use crate::render::postfx::QualityPreset;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// TOP BAR — a classic menu bar (File / About / Config) over a minimal transport
/// row (Play / Stop). Always visible. Scene I/O and quality live in the menus;
/// the transport row is just play-state.
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

/// File / About / Config menus.
fn draw_menu_bar(
    editor: &mut EditorUi,
    ui: &mut egui::Ui,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
) {
    egui::menu::bar(ui, |ui| {
        ui.menu_button(format!("{}  File", icon::FILE), |ui| {
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

        ui.menu_button(format!("{}  About", icon::INFO), |ui| {
            if ui.button("About rusty").clicked() {
                editor.show_about = true;
                ui.close_menu();
            }
        });
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
