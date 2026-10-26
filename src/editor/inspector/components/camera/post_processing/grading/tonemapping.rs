//! Tonemapping: the operator, then the display-gamma tweak.

use super::super::{knob, knob_row, Section, COLOR_CORRECTION};
use crate::scene::authoring::rendering::post_processing::grading::tonemapping;
use crate::scene::{PostProcessingComponent, Tonemap};

pub(in super::super) const SECTION: Section = Section {
    effect: "tonemapping",
    group: COLOR_CORRECTION,
    order: 24,
    draw,
};

fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let mut tonemap = vc.tonemapping.mode;
    ui.horizontal(|ui| {
        ui.label("  Tonemap:");
        egui::ComboBox::from_id_salt("tonemap")
            .selected_text(tonemap.name())
            .show_ui(ui, |ui| {
                for value in Tonemap::ALL {
                    if ui
                        .selectable_value(&mut tonemap, value, value.name())
                        .changed()
                    {
                        if let Some(mut c) = world.post_processing_mut(id) {
                            tonemapping::set_mode(&mut c, tonemap);
                        }
                        *is_dirty = true;
                    }
                }
            });
    });
    let row = (
        "  Display Gamma:",
        knob(tonemapping::EFFECT.knobs, "display_gamma"),
    );
    *is_dirty |= knob_row(ui, (world, id), row, vc, 0.5..=2.0);
}
