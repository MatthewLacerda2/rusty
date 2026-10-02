use egui_phosphor::regular as icon;

mod scene_io;
mod transport;

use crate::core::quality::QualityPreset;
use crate::editor::menu_create;
use crate::editor::EditorUi;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// TOP BAR — a classic menu bar (File / GameObject / Config / About) over the
/// toolbar row (scene name, centred Play / Stop). Always visible. Scene I/O, object
/// creation and quality live in the menus; the toolbar row is just play-state.
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
                .fill(t.bg_tier1)
                .inner_margin(egui::Margin::symmetric(t.space_sm, t.space_xs))
                .stroke(egui::Stroke::new(1.0, t.border)),
        )
        .show(ctx, |ui| {
            draw_menu_bar(editor, ui, scene, console);
            ui.add_space(2.0);
            transport::draw(editor, ui, scene, is_playing);
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

        ui.menu_button("About", |ui| {
            if ui.button("About rusty").clicked() {
                editor.show_about = true;
                ui.close_menu();
            }
        });
    });
}

/// The File menu — scene lifecycle (new / reset / load / save) and Build Settings.
fn file_menu(
    editor: &mut EditorUi,
    ui: &mut egui::Ui,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
) {
    ui.menu_button("File", |ui| {
        if ui.button(format!("{}  New Scene", icon::FILE)).clicked() {
            scene_io::new_scene(editor, scene, console);
            ui.close_menu();
        }
        if ui
            .button(format!("{}  Reset Scene", icon::ARROW_COUNTER_CLOCKWISE))
            .clicked()
        {
            scene_io::reset_scene(editor, scene, console);
            ui.close_menu();
        }
        if ui
            .button(format!("{}  Load Scene", icon::FOLDER_OPEN))
            .clicked()
        {
            scene_io::load_scene(editor, scene, console);
            ui.close_menu();
        }
        if ui
            .button(format!("{}  Save Scene", icon::FLOPPY_DISK))
            .clicked()
        {
            scene_io::save_scene(editor, scene, console);
            ui.close_menu();
        }
        ui.separator();
        if ui
            .button(format!("{}  Build Settings", icon::PACKAGE))
            .clicked()
        {
            editor.show_build_settings = true;
            ui.close_menu();
        }
    });
}

/// The Config menu — video/quality presets, the speaker mode and Scene Settings.
fn config_menu(editor: &mut EditorUi, ui: &mut egui::Ui) {
    ui.menu_button("Config", |ui| {
        ui.label("Video / Quality");
        ui.selectable_value(&mut editor.quality_preset, QualityPreset::Low, "Low");
        ui.selectable_value(&mut editor.quality_preset, QualityPreset::Medium, "Medium");
        ui.selectable_value(&mut editor.quality_preset, QualityPreset::High, "High");
        ui.separator();
        ui.label("Audio / Speaker mode");
        for mode in crate::audio::SpeakerMode::ALL {
            if ui
                .selectable_label(editor.speaker_mode == mode, mode.label())
                .clicked()
            {
                editor.speaker_mode_request = Some(mode);
            }
        }
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
