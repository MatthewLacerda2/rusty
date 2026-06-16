//! src/editor/inspector_settings.rs — the Scene Settings inspector panel.
//!
//! Shown when nothing is selected: the project-level scalars (skybox, ambient
//! light) plus the **Tags & Layers** surface that renames the shared Layers
//! registry (#90). Layer 0 is the fixed `"Default"`; slots 1..31 are user-named
//! and persist with the scene.

use egui_phosphor::regular as icon;
use glam::Vec3;

use crate::editor::theme;
use crate::editor::EditorUi;
use crate::scene::{Scene, LAYER_COUNT};

/// Render the Scene Settings panel (skybox, ambient light, Tags & Layers).
pub fn draw(editor: &mut EditorUi, ui: &mut egui::Ui, scene: &mut Scene) {
    let t = theme::from_ui(ui);
    ui.heading(format!("{}  Scene Settings", icon::GLOBE));
    ui.separator();
    ui.add_space(5.0);

    // 1. Skybox Path
    ui.horizontal(|ui| {
        ui.label("Skybox:");
        let response = ui.text_edit_singleline(&mut scene.skybox_path);
        if response.changed() {
            editor.is_dirty = true;
        }
    });
    ui.colored_label(
        t.text_secondary,
        "Provide a path to a panoramic image\n(e.g. assets/textures/sky.png)",
    );
    ui.add_space(8.0);

    // 2. Ambient Light Color
    ui.label("Ambient Color:");
    ui.horizontal(|ui| {
        let mut color_arr = [
            scene.ambient_color.x,
            scene.ambient_color.y,
            scene.ambient_color.z,
        ];
        if ui.color_edit_button_rgb(&mut color_arr).changed() {
            scene.ambient_color = Vec3::new(color_arr[0], color_arr[1], color_arr[2]);
            editor.is_dirty = true;
        }
    });
    ui.add_space(8.0);

    // 3. Ambient Light Intensity
    ui.label("Ambient Intensity:");
    let response =
        ui.add(egui::Slider::new(&mut scene.ambient_intensity, 0.0..=5.0).text("intensity"));
    if response.changed() {
        editor.is_dirty = true;
    }
    ui.add_space(8.0);

    // 4. Tags & Layers — rename the project's shared layer slots, referenced by an
    // entity's Layer dropdown in the entity inspector.
    draw_layers(editor, ui, scene);

    ui.add_space(15.0);
    ui.separator();
    ui.add_space(5.0);
    ui.colored_label(
        t.text_secondary,
        "Select an entity from Hierarchy\nor click a project asset to inspect.",
    );
}

/// The Tags & Layers surface: rename the project's 32 layer slots. Layer 0 is the
/// fixed `"Default"`; the rest are editable. Names persist with the scene.
fn draw_layers(editor: &mut EditorUi, ui: &mut egui::Ui, scene: &mut Scene) {
    let t = theme::from_ui(ui);
    egui::CollapsingHeader::new(format!("{}  Tags & Layers", icon::STACK)).show(ui, |ui| {
        ui.colored_label(t.text_secondary, "Layer 0 is the fixed Default layer.");
        ui.horizontal(|ui| {
            ui.label("0:");
            ui.colored_label(t.text_secondary, scene.layers.label(0));
        });
        for i in 1..LAYER_COUNT as u8 {
            // Seed an editable buffer from the model each frame; commit via
            // `set_name` so layer 0 stays protected and names are trimmed.
            let mut name = scene.layers.name(i).to_string();
            ui.horizontal(|ui| {
                ui.label(format!("{i}:"));
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut name)
                        .hint_text(format!("Layer {i}"))
                        .desired_width(f32::INFINITY),
                );
                if resp.changed() {
                    scene.layers.set_name(i, name);
                    editor.is_dirty = true;
                }
            });
        }
    });
}
