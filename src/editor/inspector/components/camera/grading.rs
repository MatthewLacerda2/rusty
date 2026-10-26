//! The colour-grading rows of the Post Processing card's Color Correction section
//! (#1440): white balance, the Lift / Gamma / Gain wheels and the hue shift. Split
//! out of `post_processing` to keep it under the size cap.
//!
//! A THIN client (#287): each widget reads the snapshot and writes through the
//! shared `authoring::rendering::post_processing` op `Graphics.*` calls too.

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
    let mut write = |f: &dyn Fn(&mut PostProcessingComponent)| {
        if let Some(mut c) = world.post_processing_mut(id) {
            f(&mut c);
        }
        *is_dirty = true;
    };
    let r = WhiteBalance::RANGE;
    let WhiteBalance {
        mut temperature,
        mut tint,
    } = vc.white_balance;
    ui.label("  White Balance");
    let t = slider(ui, "    Temperature:", &mut temperature, -r..=r);
    let n = slider(ui, "    Tint:", &mut tint, -r..=r);
    if t || n {
        write(&|c| vc_ops::set_white_balance(c, temperature, tint));
    }
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
            write(&|c| vc_ops::set_wheel(c, which, wheel.color, wheel.offset));
        }
    }
    let mut hue = vc.hue_shift;
    if slider(ui, "  Hue Shift:", &mut hue, -180.0..=180.0) {
        write(&|c| vc_ops::set_hue_shift(c, hue));
    }
}

/// A labelled slider; whether it changed.
fn slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::Slider::new(value, range)).changed()
    })
    .inner
}
