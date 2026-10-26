//! Chromatic Aberration: on/off and intensity.

use super::super::{knob, knob_row, toggle, Section, LENS_EFFECTS};
use crate::scene::authoring::rendering::post_processing::lens::chromatic_aberration as ca;
use crate::scene::PostProcessingComponent;

pub(in super::super) const SECTION: Section = Section {
    effect: "chromatic_aberration",
    group: LENS_EFFECTS,
    order: 60,
    draw,
};

fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let on = vc.chromatic_aberration.active;
    toggle(
        ui,
        (&mut *world, id),
        "  Chromatic Aberration",
        on,
        ca::set_active,
        is_dirty,
    );
    if on {
        let row = ("    Intensity:", knob(ca::EFFECT.knobs, "intensity"));
        *is_dirty |= knob_row(ui, (world, id), row, vc, 0.0..=1.0);
    }
}
