//! Film Grain: on/off, intensity and response.

use super::super::{knob, knob_row, toggle, Section, LENS_EFFECTS};
use rusty_edit::authoring::rendering::post_processing::lens::film_grain;
use rusty_scene::PostProcessingComponent;

pub(in super::super) const SECTION: Section = Section {
    effect: "film_grain",
    group: LENS_EFFECTS,
    order: 62,
    draw,
};

fn draw(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, vc: &PostProcessingComponent) {
    let on = vc.film_grain.active;
    toggle(
        ui,
        (&mut *world, id),
        "  Film Grain",
        on,
        film_grain::set_active,
    );
    if on {
        for (label, field) in [
            ("    Intensity:", "intensity"),
            ("    Response:", "response"),
        ] {
            let row = (label, knob(film_grain::EFFECT.knobs, field));
            knob_row(ui, (&mut *world, id), row, vc, 0.0..=1.0);
        }
    }
}
