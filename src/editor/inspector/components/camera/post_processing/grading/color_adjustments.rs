//! Color adjustments: contrast, saturation and the hue shift, each through its
//! knob.

use super::super::{knob, knob_row, Section, COLOR_CORRECTION};
use crate::scene::authoring::rendering::post_processing::grading::color_adjustments;
use crate::scene::PostProcessingComponent;

pub(in super::super) const SECTION: Section = Section {
    effect: "color_adjustments",
    group: COLOR_CORRECTION,
    order: 23,
    draw,
};

fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let rows = [
        ("  Contrast:", "contrast", 0.0..=2.0),
        ("  Saturation:", "saturation", 0.0..=2.0),
        ("  Hue Shift:", "hue_shift", -180.0..=180.0),
    ];
    for (label, field, range) in rows {
        let row = (label, knob(color_adjustments::EFFECT.knobs, field));
        *is_dirty |= knob_row(ui, (&mut *world, id), row, vc, range);
    }
}
