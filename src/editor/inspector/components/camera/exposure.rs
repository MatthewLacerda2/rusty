//! The exposure rows of the Post Processing card's Color Correction section
//! (#1444): the mode, then the fixed EV or the automatic mode's limits,
//! compensation, adaptation speeds and histogram window. Split out of
//! `post_processing` to keep it under the size cap.
//!
//! A THIN client (#287): each widget reads the snapshot and writes through the
//! shared `authoring::rendering::post_processing` op `Graphics.*` calls too.

use super::post_processing::slider_row;
use crate::components::{AutoExposure, ExposureMode};
use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::PostProcessingComponent;

/// The mode selector and the rows of the selected mode.
#[rustfmt::skip]
pub(super) fn draw_exposure(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    draw_mode(ui, world, id, vc, is_dirty);
    if vc.exposure.mode == ExposureMode::Fixed {
        *is_dirty |= slider_row(ui, (&mut *world, id), "  Exposure (EV):", vc.exposure.fixed_ev, -4.0..=4.0, vc_ops::set_exposure);
        return;
    }
    let (ae, r) = (vc.exposure.automatic, AutoExposure::EV_RANGE);
    *is_dirty |= slider_row(ui, (&mut *world, id), "    Min EV:", ae.min_ev, -r..=r, set_min_ev);
    *is_dirty |= slider_row(ui, (&mut *world, id), "    Max EV:", ae.max_ev, -r..=r, set_max_ev);
    *is_dirty |= slider_row(ui, (&mut *world, id), "    Compensation:", ae.compensation, -r..=r, vc_ops::set_exposure_compensation);
    *is_dirty |= slider_row(ui, (&mut *world, id), "    Speed Up (EV/s):", ae.speed_up, 0.0..=AutoExposure::MAX_SPEED, set_speed_up);
    *is_dirty |= slider_row(ui, (&mut *world, id), "    Speed Down (EV/s):", ae.speed_down, 0.0..=AutoExposure::MAX_SPEED, set_speed_down);
    *is_dirty |= slider_row(ui, (&mut *world, id), "    Low Percent:", ae.low_percent, 0.0..=100.0, set_low);
    *is_dirty |= slider_row(ui, (&mut *world, id), "    High Percent:", ae.high_percent, 0.0..=100.0, set_high);
}

/// The Fixed / Automatic selector.
fn draw_mode(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let mut mode = vc.exposure.mode;
    ui.horizontal(|ui| {
        ui.label("  Exposure:");
        egui::ComboBox::from_id_salt("exposure_mode")
            .selected_text(mode.name())
            .show_ui(ui, |ui| {
                for value in ExposureMode::ALL {
                    if ui
                        .selectable_value(&mut mode, value, value.name())
                        .changed()
                    {
                        if let Some(mut c) = world.post_processing_mut(id) {
                            vc_ops::set_exposure_mode(&mut c, mode);
                        }
                        *is_dirty = true;
                    }
                }
            });
    });
}

// One half of each pair the ops set together, the other half kept.

fn set_min_ev(vc: &mut PostProcessingComponent, v: f32) {
    vc_ops::set_exposure_limits(vc, v, vc.exposure.automatic.max_ev);
}

fn set_max_ev(vc: &mut PostProcessingComponent, v: f32) {
    vc_ops::set_exposure_limits(vc, vc.exposure.automatic.min_ev, v);
}

fn set_speed_up(vc: &mut PostProcessingComponent, v: f32) {
    vc_ops::set_adaptation_speed(vc, v, vc.exposure.automatic.speed_down);
}

fn set_speed_down(vc: &mut PostProcessingComponent, v: f32) {
    vc_ops::set_adaptation_speed(vc, vc.exposure.automatic.speed_up, v);
}

fn set_low(vc: &mut PostProcessingComponent, v: f32) {
    vc_ops::set_histogram_percent(vc, v, vc.exposure.automatic.high_percent);
}

fn set_high(vc: &mut PostProcessingComponent, v: f32) {
    vc_ops::set_histogram_percent(vc, vc.exposure.automatic.low_percent, v);
}
