//! White balance: temperature and tint, each through its knob.

use super::super::{knob, knob_row, Section, COLOR_CORRECTION};
use crate::components::WhiteBalance;
use crate::scene::authoring::rendering::post_processing::grading::white_balance;
use crate::scene::PostProcessingComponent;

pub(in super::super) const SECTION: Section = Section {
    effect: "white_balance",
    group: COLOR_CORRECTION,
    order: 21,
    draw,
};

fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let r = WhiteBalance::RANGE;
    ui.label("  White Balance");
    for (label, field) in [("    Temperature:", "temperature"), ("    Tint:", "tint")] {
        let row = (label, knob(white_balance::EFFECT.knobs, field));
        *is_dirty |= knob_row(ui, (&mut *world, id), row, vc, -r..=r);
    }
}
