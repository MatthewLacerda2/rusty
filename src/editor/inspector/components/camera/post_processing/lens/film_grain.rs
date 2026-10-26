//! Film Grain: on/off, intensity and response.

use super::super::{knob, knob_row, toggle, Section, LENS_EFFECTS};
use crate::scene::authoring::rendering::post_processing::lens::film_grain;
use crate::scene::PostProcessingComponent;

pub(in super::super) const SECTION: Section = Section {
    effect: "film_grain",
    group: LENS_EFFECTS,
    order: 62,
    draw,
};

fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let on = vc.film_grain.active;
    toggle(
        ui,
        (&mut *world, id),
        "  Film Grain",
        on,
        film_grain::set_active,
        is_dirty,
    );
    if on {
        for (label, field) in [
            ("    Intensity:", "intensity"),
            ("    Response:", "response"),
        ] {
            let row = (label, knob(film_grain::EFFECT.knobs, field));
            *is_dirty |= knob_row(ui, (&mut *world, id), row, vc, 0.0..=1.0);
        }
    }
}
