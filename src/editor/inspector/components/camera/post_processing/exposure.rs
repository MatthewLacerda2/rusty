//! The exposure rows of the Post Processing card's Color Correction section
//! (#1444): the mode, then the fixed EV or the automatic mode's limits,
//! compensation, adaptation speeds and histogram window, each through its knob.

use super::{knob, knob_row, Section, COLOR_CORRECTION};
use crate::components::{AutoExposure, ExposureMode};
use crate::scene::authoring::rendering::post_processing::exposure;
use crate::scene::PostProcessingComponent;

pub(super) const SECTION: Section = Section {
    effect: "exposure",
    group: COLOR_CORRECTION,
    order: 20,
    draw,
};

/// The mode selector and the rows of the selected mode.
fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    draw_mode(ui, world, id, vc, is_dirty);
    let (r, speed) = (AutoExposure::EV_RANGE, AutoExposure::MAX_SPEED);
    let rows: &[(&str, &str, (f32, f32))] = match vc.exposure.mode {
        ExposureMode::Fixed => &[("  Exposure (EV):", "fixed_ev", (-4.0, 4.0))],
        ExposureMode::Automatic => &[
            ("    Min EV:", "automatic.min_ev", (-r, r)),
            ("    Max EV:", "automatic.max_ev", (-r, r)),
            ("    Compensation:", "automatic.compensation", (-r, r)),
            ("    Speed Up (EV/s):", "automatic.speed_up", (0.0, speed)),
            (
                "    Speed Down (EV/s):",
                "automatic.speed_down",
                (0.0, speed),
            ),
            ("    Low Percent:", "automatic.low_percent", (0.0, 100.0)),
            ("    High Percent:", "automatic.high_percent", (0.0, 100.0)),
        ],
    };
    for &(label, field, (lo, hi)) in rows {
        let row = (label, knob(exposure::EFFECT.knobs, field));
        *is_dirty |= knob_row(ui, (&mut *world, id), row, vc, lo..=hi);
    }
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
                            exposure::set_mode(&mut c, mode);
                        }
                        *is_dirty = true;
                    }
                }
            });
    });
}
