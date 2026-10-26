//! The Color Lookup (#1441): the LUT file ([`super::lut_field`]) and its
//! contribution.

use super::super::{knob, knob_row, Section, COLOR_CORRECTION};
use crate::scene::authoring::rendering::post_processing::grading::color_lookup;
use crate::scene::PostProcessingComponent;

pub(in super::super) const SECTION: Section = Section {
    effect: "color_lookup",
    group: COLOR_CORRECTION,
    order: 25,
    draw,
};

fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    ui.label("  Color Lookup");
    super::lut_field::draw(ui, world, id, vc, is_dirty);
    let row = (
        "    Contribution:",
        knob(color_lookup::EFFECT.knobs, "contribution"),
    );
    *is_dirty |= knob_row(ui, (world, id), row, vc, 0.0..=1.0);
}
