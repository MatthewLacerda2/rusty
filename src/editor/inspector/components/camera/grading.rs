//! The colour-grading rows of the Post Processing card's Color Correction section
//! (#1440): white balance, the Lift / Gamma / Gain wheels and the hue shift, and
//! the Color Lookup (#1441). Split out of `post_processing` to keep it under the
//! size cap.
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
    let row = (&mut *world, id);
    *is_dirty |= slider_row(
        ui,
        row,
        "    Temperature:",
        wb.temperature,
        -r..=r,
        set_temperature,
    );
    let row = (&mut *world, id);
    *is_dirty |= slider_row(ui, row, "    Tint:", wb.tint, -r..=r, set_tint);
    draw_wheels(ui, world, id, vc, is_dirty);
    let (row, hue) = ((world, id), vc.hue_shift);
    *is_dirty |= slider_row(
        ui,
        row,
        "  Hue Shift:",
        hue,
        -180.0..=180.0,
        vc_ops::set_hue_shift,
    );
}

/// The Lift / Gamma / Gain wheels: a colour and an offset each.
fn draw_wheels(
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
                vc_ops::set_wheel(&mut c, which, wheel.color, wheel.offset);
            }
            *is_dirty = true;
        }
    }
}

/// The Color Lookup: the LUT file and its contribution. The path is written when
/// the field loses focus (Enter or a click away), not per keystroke, so the
/// renderer never tries, and logs, every half-typed path.
pub(super) fn draw_color_lookup(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    ui.label("  Color Lookup");
    let key = egui::Id::new(("color_lookup", id));
    let saved = vc.lut.clone().unwrap_or_default();
    let mut text = ui
        .data_mut(|d| d.get_temp::<String>(key))
        .unwrap_or(saved.clone());
    let field = ui
        .horizontal(|ui| {
            ui.label("    LUT:");
            ui.text_edit_singleline(&mut text)
                .on_hover_text("A .cube, or a Unity strip .png (256×16, 1024×32)")
        })
        .inner;
    if field.has_focus() {
        ui.data_mut(|d| d.insert_temp(key, text));
    } else {
        ui.data_mut(|d| d.remove::<String>(key));
        if field.lost_focus() && text != saved {
            if let Some(mut c) = world.post_processing_mut(id) {
                vc_ops::set_lut(&mut c, Some(text));
            }
            *is_dirty = true;
        }
    }
    let k = vc.lut_contribution;
    let set = vc_ops::set_lut_contribution;
    *is_dirty |= slider_row(ui, (world, id), "    Contribution:", k, 0.0..=1.0, set);
}

fn set_temperature(c: &mut PostProcessingComponent, temperature: f32) {
    let tint = c.white_balance.tint;
    vc_ops::set_white_balance(c, temperature, tint);
}

fn set_tint(c: &mut PostProcessingComponent, tint: f32) {
    let temperature = c.white_balance.temperature;
    vc_ops::set_white_balance(c, temperature, tint);
}
