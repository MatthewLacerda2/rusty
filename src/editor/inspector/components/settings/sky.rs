//! The Scene Settings panel's procedural sky section (#1620): the zenith, horizon and
//! ground colours and the cloud coverage and opacity, each written through its
//! `authoring::environment` op, the one `Lighting.Set*` calls.

use egui_phosphor::regular as icon;

use crate::editor::EditorUi;
use crate::scene::authoring::environment as env_ops;
use crate::scene::{Scene, SkySettings};

/// A colour's label, what it reads when unset, and its op.
type ColorRow = (&'static str, &'static str, fn(&mut SkySettings, Option<glam::Vec3>));

/// The procedural sky section, collapsed by default like Fog.
pub(super) fn draw(editor: &mut EditorUi, ui: &mut egui::Ui, scene: &mut Scene) {
    let before = scene.sky;
    let resolved = before.resolved(scene.ambient_color);
    let sky = &mut scene.sky;
    let mut changed = false;
    egui::CollapsingHeader::new(format!("{}  Procedural Sky", icon::CLOUD_SUN)).show(ui, |ui| {
        let rows: [(ColorRow, Option<glam::Vec3>); 3] = [
            (("Zenith", "follows Ambient Color", env_ops::set_sky_zenith), before.zenith),
            (("Horizon", "follows Zenith", env_ops::set_sky_horizon), before.horizon),
            (("Ground", "follows Zenith", env_ops::set_sky_ground), before.ground),
        ];
        for (((label, unset, op), current), shown) in rows.into_iter().zip(resolved) {
            changed |= color_row(ui, sky, (label, unset, op), (current, shown));
        }
        changed |= cloud_sliders(ui, sky, &before);
    });
    editor.is_dirty |= changed;
}

/// One colour: a checkbox sets it (from the colour drawn now) or lets it follow
/// again, and a picker edits it while set. Returns whether it changed.
fn color_row(
    ui: &mut egui::Ui,
    sky: &mut SkySettings,
    (label, unset, op): ColorRow,
    (current, shown): (Option<glam::Vec3>, glam::Vec3),
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let mut set = current.is_some();
        if ui.checkbox(&mut set, label).changed() {
            op(sky, set.then_some(shown));
            changed = true;
        }
        match current {
            Some(c) => {
                let mut rgb = c.to_array();
                if ui.color_edit_button_rgb(&mut rgb).changed() {
                    op(sky, Some(glam::Vec3::from_array(rgb)));
                    changed = true;
                }
            }
            None => {
                ui.weak(unset);
            }
        }
    });
    changed
}

/// The cloud coverage and opacity sliders. Returns whether either changed.
fn cloud_sliders(ui: &mut egui::Ui, sky: &mut SkySettings, before: &SkySettings) -> bool {
    type Op = fn(&mut SkySettings, f32);
    let sliders: [(&str, f32, Op); 2] = [
        ("Cloud Coverage", before.cloud_coverage, env_ops::set_cloud_coverage),
        ("Cloud Opacity", before.cloud_opacity, env_ops::set_cloud_opacity),
    ];
    let mut changed = false;
    for (label, mut value, op) in sliders {
        if ui
            .add(crate::editor::widgets::slider(&mut value, 0.0..=1.0).text(label))
            .changed()
        {
            op(sky, value);
            changed = true;
        }
    }
    changed
}
