mod ambient;
mod sky;

use egui_phosphor::regular as icon;
use glam::Vec3;

use crate::theme;
use crate::EditorUi;
use rusty_edit::authoring::environment as env_ops;
use rusty_edit::authoring::fog as fog_ops;
use rusty_scene::{FogMode, FogSettings, Scene, LAYER_COUNT};

/// Render the Scene Settings panel (skybox, ambient light, Tags & Layers).
pub fn draw(editor: &mut EditorUi, ui: &mut egui::Ui, scene: &mut Scene) {
    let t = theme::from_ui(ui);
    ui.heading(format!("{}  Scene Settings", icon::GLOBE));
    ui.separator();
    ui.add_space(5.0);

    draw_environment(editor, ui, scene);

    sky::draw(editor, ui, scene);

    draw_fog(editor, ui, scene);
    ui.add_space(8.0);

    draw_layers(editor, ui, scene);
    ui.add_space(8.0);

    draw_collision_matrix(editor, ui, scene);

    ui.add_space(15.0);
    ui.separator();
    ui.add_space(5.0);
    ui.colored_label(
        t.text_secondary,
        "Select an entity from Hierarchy\nor click a project asset to inspect.",
    );
}

/// The skybox path and the ambient colour and intensity, each written through its
/// `authoring::environment` op, the one `Lighting.Set*` calls.
fn draw_environment(editor: &mut EditorUi, ui: &mut egui::Ui, scene: &mut Scene) {
    let t = theme::from_ui(ui);
    ui.horizontal(|ui| {
        ui.label("Skybox:");
        let mut path = scene.skybox_path.clone();
        if ui.text_edit_singleline(&mut path).changed() {
            env_ops::set_skybox(scene, &path);
            editor.is_dirty = true;
        }
    });
    ui.colored_label(
        t.text_secondary,
        "Provide a path to a panoramic image\n(e.g. assets/textures/sky.png)",
    );
    ui.add_space(8.0);

    ambient::draw(editor, ui, scene);
    ui.add_space(8.0);

    ui.label("Ambient Intensity:");
    let mut intensity = scene.ambient_intensity;
    let response = ui.add(crate::widgets::slider(&mut intensity, 0.0..=5.0).text("intensity"));
    if response.changed() {
        env_ops::set_ambient_intensity(scene, intensity);
        editor.is_dirty = true;
    }
    ui.add_space(8.0);
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

/// The scene fog section. A thin client: each widget edits a snapshot and
/// routes the write through the shared `authoring::fog` op the `Graphics.SetFog*`
/// bindings call, so panel and API share one clamp.
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
            .add(crate::widgets::slider(&mut value, range).text(label))
            .changed()
        {
            op(fog, value);
            changed = true;
        }
    }
    changed
}
