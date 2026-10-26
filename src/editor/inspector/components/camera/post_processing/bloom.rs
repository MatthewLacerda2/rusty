//! The Bloom section: the switch and, while on, its two knobs.

use super::{knob, knob_row, toggle, Section};
use crate::scene::authoring::rendering::post_processing::bloom;
use crate::scene::PostProcessingComponent;

pub(super) const SECTION: Section = Section {
    effect: "bloom",
    group: None,
    order: 10,
    draw,
};

fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Bloom");
    let on = vc.bloom.active;
    toggle(
        ui,
        (&mut *world, id),
        "  Bloom Active",
        on,
        bloom::set_active,
        is_dirty,
    );
    if on {
        let knobs = bloom::EFFECT.knobs;
        let rows = [
            ("    Intensity:", "intensity", 5.0),
            ("    Threshold:", "threshold", 1.0),
        ];
        for (label, field, max) in rows {
            let row = (label, knob(knobs, field));
            *is_dirty |= knob_row(ui, (&mut *world, id), row, vc, 0.0..=max);
        }
    }
}
