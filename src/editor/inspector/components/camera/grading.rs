//! The colour-grading rows of the Post Processing card's Color Correction section
//! (#1440): white balance, the Lift / Gamma / Gain wheels and the hue shift. Split
//! out of `post_processing` to keep it under the size cap.
//!
//! A THIN client (#287): each widget reads the snapshot and writes through the
//! shared `authoring::rendering::post_processing` op `Graphics.*` calls too.

use super::post_processing::slider_row;
use crate::components::{Wheel, WhiteBalance};
use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::PostProcessingComponent;

/// White balance, the three wheels and the hue shift.
pub(super) fn draw_grading(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let r = WhiteBalance::RANGE;
    let wb = vc.white_balance;
    ui.label("  White Balance");
    *is_dirty |= slider_row(
        ui,
        (&mut *world, id),
        "    Temperature:",
        wb.temperature,
        -r..=r,
        |c, t| {
            let tint = c.white_balance.tint;
            vc_ops::set_white_balance(c, t, tint);
        },
    );
    *is_dirty |= slider_row(
        ui,
        (&mut *world, id),
        "    Tint:",
        wb.tint,
        -r..=r,
        |c, n| {
            let temperature = c.white_balance.temperature;
            vc_ops::set_white_balance(c, temperature, n);
        },
    );
    ui.label("  Lift / Gamma / Gain");
    for which in Wheel::ALL {
        let mut wheel = *vc.lift_gamma_gain.wheel(which);
        let changed = ui
            .horizontal(|ui| {
                ui.label(format!("    {}:", which.name()));
                let colour = ui.color_edit_button_rgb(&mut wheel.color).changed();
                let offset = egui::Slider::new(&mut wheel.offset, -1.0..=1.0);
                colour | ui.add(offset).changed()
            })
            .inner;
        if changed {
            if let Some(mut c) = world.post_processing_mut(id) {
                vc_ops::set_wheel(&mut c, which, wheel.color, wheel.offset);
            }
            *is_dirty = true;
        }
    }
    let hue = vc.hue_shift;
    *is_dirty |= slider_row(
        ui,
        (&mut *world, id),
        "  Hue Shift:",
        hue,
        -180.0..=180.0,
        vc_ops::set_hue_shift,
    );
}
