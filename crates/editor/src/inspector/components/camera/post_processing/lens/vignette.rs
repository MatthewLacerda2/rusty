//! Vignette: on/off, intensity, smoothness, colour and centre (one row per axis:
//! two sliders overflow the inspector's width).

use super::super::{knob, knob_row, toggle, Section, LENS_EFFECTS};
use rusty_components::Vignette;
use rusty_edit::authoring::rendering::post_processing::lens::vignette;
use rusty_scene::PostProcessingComponent;

pub(in super::super) const SECTION: Section = Section {
    effect: "vignette",
    group: LENS_EFFECTS,
    order: 61,
    draw,
};

fn draw(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, vc: &PostProcessingComponent) {
    let on = vc.vignette.active;
    toggle(
        ui,
        (&mut *world, id),
        "  Vignette",
        on,
        vignette::set_active,
    );
    if !on {
        return;
    }
    let knobs = vignette::EFFECT.knobs;
    let smooth = Vignette::MIN_SMOOTHNESS..=1.0;
    let row = ("    Intensity:", knob(knobs, "intensity"));
    knob_row(ui, (&mut *world, id), row, vc, 0.0..=1.0);
    let row = ("    Smoothness:", knob(knobs, "smoothness"));
    knob_row(ui, (&mut *world, id), row, vc, smooth);
    let mut color = vc.vignette.color;
    let changed = ui
        .horizontal(|ui| {
            ui.label("    Color:");
            ui.color_edit_button_rgb(&mut color).changed()
        })
        .inner;
    if changed {
        if let Some(mut c) = world.post_processing_mut(id) {
            vignette::set_color(&mut c, color);
        }
    }
    for (label, field) in [("    Center X:", "center.x"), ("    Center Y:", "center.y")] {
        let row = (label, knob(knobs, field));
        knob_row(ui, (&mut *world, id), row, vc, 0.0..=1.0);
    }
}
