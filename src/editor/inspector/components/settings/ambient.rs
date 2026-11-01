//! The Scene Settings panel's Environment Lighting (#1647): Unity's *Source*
//! dropdown, `Color` or `Gradient`, the ambient colour, and the gradient's sky,
//! equator and ground colours, each written through its `authoring::environment`
//! op, the one `Lighting.Set*` calls.

use glam::Vec3;

use crate::editor::EditorUi;
use crate::scene::authoring::environment as env_ops;
use crate::scene::{AmbientMode, AmbientSettings, Scene};

/// A gradient colour's label, its value and its op.
type GradientRow = (&'static str, Vec3, fn(&mut AmbientSettings, Vec3));

/// The source dropdown, the ambient colour, and in `Gradient` mode its three colours.
pub(super) fn draw(editor: &mut EditorUi, ui: &mut egui::Ui, scene: &mut Scene) {
    let before = scene.ambient;
    let mut changed = false;
    egui::ComboBox::from_label("Ambient Source")
        .selected_text(before.mode.name())
        .show_ui(ui, |ui| {
            for mode in AmbientMode::ALL {
                if ui
                    .selectable_label(before.mode == mode, mode.name())
                    .clicked()
                    && before.mode != mode
                {
                    env_ops::set_ambient_mode(&mut scene.ambient, mode);
                    changed = true;
                }
            }
        });
    // In Gradient mode the colour still tints the procedural sky, so it stays.
    let label = match before.mode {
        AmbientMode::Color => "Ambient Color:",
        AmbientMode::Gradient => "Ambient Color (tints the sky):",
    };
    ui.horizontal(|ui| {
        ui.label(label);
        let mut rgb = scene.ambient_color.to_array();
        if ui.color_edit_button_rgb(&mut rgb).changed() {
            env_ops::set_ambient_color(scene, Vec3::from_array(rgb));
            changed = true;
        }
    });
    if before.mode == AmbientMode::Gradient {
        changed |= gradient_rows(ui, &mut scene.ambient, &before);
    }
    editor.is_dirty |= changed;
}

/// The gradient's sky, equator and ground colour pickers. Returns whether one changed.
fn gradient_rows(
    ui: &mut egui::Ui,
    ambient: &mut AmbientSettings,
    before: &AmbientSettings,
) -> bool {
    let rows: [GradientRow; 3] = [
        ("Sky Color:", before.sky, env_ops::set_ambient_sky_color),
        (
            "Equator Color:",
            before.equator,
            env_ops::set_ambient_equator_color,
        ),
        (
            "Ground Color:",
            before.ground,
            env_ops::set_ambient_ground_color,
        ),
    ];
    let mut changed = false;
    for (label, value, op) in rows {
        ui.horizontal(|ui| {
            ui.label(label);
            let mut rgb = value.to_array();
            if ui.color_edit_button_rgb(&mut rgb).changed() {
                op(ambient, Vec3::from_array(rgb));
                changed = true;
            }
        });
    }
    changed
}
