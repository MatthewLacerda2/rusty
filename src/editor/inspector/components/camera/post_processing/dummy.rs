//! The test-only dummy effect's section (#1467): its switch and, while on, its knob.

use super::{knob, knob_row, toggle, Section};
use crate::scene::authoring::rendering::post_processing::dummy;
use crate::scene::PostProcessingComponent;

pub(super) const SECTION: Section = Section {
    effect: "dummy",
    group: None,
    order: 90,
    draw,
};

fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let on = vc.dummy.active;
    toggle(
        ui,
        (&mut *world, id),
        "Dummy",
        on,
        dummy::set_active,
        is_dirty,
    );
    if on {
        let row = ("  Strength:", knob(dummy::EFFECT.knobs, "strength"));
        *is_dirty |= knob_row(ui, (world, id), row, vc, 0.0..=1.0);
    }
}
