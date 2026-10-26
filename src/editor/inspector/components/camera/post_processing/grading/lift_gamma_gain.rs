//! Lift / Gamma / Gain: a colour and an offset per wheel.

use super::super::{Section, COLOR_CORRECTION};
use crate::components::Wheel;
use crate::scene::authoring::rendering::post_processing::grading::lift_gamma_gain;
use crate::scene::PostProcessingComponent;

pub(in super::super) const SECTION: Section = Section {
    effect: "lift_gamma_gain",
    group: COLOR_CORRECTION,
    order: 22,
    draw,
};

fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    ui.label("  Lift / Gamma / Gain");
    for which in Wheel::ALL {
        let mut wheel = *vc.lift_gamma_gain.wheel(which);
        let changed = ui
            .horizontal(|ui| {
                ui.label(format!("    {}:", which.name()));
                let colour = ui.color_edit_button_rgb(&mut wheel.color).changed();
                let offset = crate::editor::widgets::slider(&mut wheel.offset, -1.0..=1.0);
                colour | ui.add(offset).changed()
            })
            .inner;
        if changed {
            if let Some(mut c) = world.post_processing_mut(id) {
                lift_gamma_gain::set_wheel(&mut c, which, wheel.color, wheel.offset);
            }
            *is_dirty = true;
        }
    }
}
