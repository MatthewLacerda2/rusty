//! src/editor/inspector_settings.rs — the Scene Settings inspector panel.
//!
//! Shown when nothing is selected: the project-level scalars (skybox, ambient
//! light), the scene fog (#437, routed through `authoring::fog` like `Graphics.SetFog*`)
//! plus the **Tags & Layers** surface that renames the shared Layers
//! registry (#90). Layer 0 is the fixed `"Default"`; slots 1..31 are user-named
//! and persist with the scene.

use egui_phosphor::regular as icon;
use glam::Vec3;

use crate::editor::theme;
use crate::editor::EditorUi;
use crate::scene::authoring::fog as fog_ops;
use crate::scene::{FogMode, FogSettings, Scene, LAYER_COUNT};

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

    // 4. Fog — distance + height, applied to every world-space pass.
    draw_fog(editor, ui, scene);
    ui.add_space(8.0);

    // 5. Tags & Layers — rename the project's shared layer slots, referenced by an
    // entity's Layer dropdown in the entity inspector.
    draw_layers(editor, ui, scene);
    ui.add_space(8.0);

    // 6. Layer Collision Matrix — which layer collides with which (#91).
    draw_collision_matrix(editor, ui, scene);

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

/// The Unity-style Layer Collision Matrix: a clickable grid over the named layers
/// (layer 0 plus any named slots). Unchecking a cell stops those two layers from
/// colliding; the matrix is symmetric and persists with the scene.
fn draw_collision_matrix(editor: &mut EditorUi, ui: &mut egui::Ui, scene: &mut Scene) {
    let t = theme::from_ui(ui);
    egui::CollapsingHeader::new(format!("{}  Layer Collision Matrix", icon::STACK)).show(
        ui,
        |ui| {
            ui.colored_label(t.text_secondary, "Unchecked pairs never collide.");
            let layers: Vec<(u8, String)> = (0..LAYER_COUNT as u8)
                .filter(|&i| i == 0 || !scene.layers.name(i).is_empty())
                .map(|i| (i, scene.layers.label(i)))
                .collect();
            egui::Grid::new("collision_matrix_grid")
                .striped(true)
                .show(ui, |ui| {
                    // Header row: a blank corner, then one column per layer (index label,
                    // full name on hover, to keep the grid compact).
                    ui.label("");
                    for (j, name) in &layers {
                        ui.label(format!("{j}")).on_hover_text(name);
                    }
                    ui.end_row();
                    for (i, name) in &layers {
                        ui.label(name);
                        for (j, _) in &layers {
                            let mut on = scene.collision_matrix.can_collide(*i, *j);
                            if ui.checkbox(&mut on, "").changed() {
                                scene.collision_matrix.set_collision(*i, *j, on);
                                editor.is_dirty = true;
                            }
                        }
                        ui.end_row();
                    }
                });
        },
    );
}

/// The scene fog section (#437). A thin client: each widget edits a snapshot and
/// routes the write through the shared `authoring::fog` op the `Graphics.SetFog*`
/// bindings call, so panel and API share one write and one clamp.
fn draw_fog(editor: &mut EditorUi, ui: &mut egui::Ui, scene: &mut Scene) {
    let f = scene.fog;
    let fog = &mut scene.fog;
    let mut changed = false;
    egui::CollapsingHeader::new(format!("{}  Fog", icon::CLOUD_FOG)).show(ui, |ui| {
        egui::ComboBox::from_label("Mode")
            .selected_text(f.mode.name())
            .show_ui(ui, |ui| {
                for mode in FogMode::ALL {
                    if ui.selectable_label(f.mode == mode, mode.name()).clicked() {
                        fog_ops::set_mode(fog, mode);
                        changed = true;
                    }
                }
            });
        let mut rgb = f.color.to_array();
        ui.horizontal(|ui| {
            ui.label("Color:");
            if ui.color_edit_button_rgb(&mut rgb).changed() {
                fog_ops::set_color(fog, Vec3::from_array(rgb));
                changed = true;
            }
        });
        changed |= draw_fog_sliders(ui, fog, &f);
    });
    editor.is_dirty |= changed;
}

/// The fog's five scalar sliders, each through its shared op. Returns whether any
/// changed.
fn draw_fog_sliders(ui: &mut egui::Ui, fog: &mut FogSettings, f: &FogSettings) -> bool {
    type Op = fn(&mut FogSettings, f32);
    let sliders: [(&str, f32, std::ops::RangeInclusive<f32>, Op); 5] = [
        ("Density", f.density, 0.0..=0.5, fog_ops::set_density),
        ("Start", f.start, 0.0..=500.0, fog_ops::set_start),
        ("End", f.end, 0.0..=1000.0, fog_ops::set_end),
        (
            "Height Falloff",
            f.height_falloff,
            0.0..=2.0,
            fog_ops::set_height_falloff,
        ),
        (
            "Base Height",
            f.base_height,
            -50.0..=50.0,
            fog_ops::set_base_height,
        ),
    ];
    let mut changed = false;
    for (label, mut value, range, op) in sliders {
        if ui
            .add(egui::Slider::new(&mut value, range).text(label))
            .changed()
        {
            op(fog, value);
            changed = true;
        }
    }
    changed
}
