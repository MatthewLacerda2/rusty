//! The Post Processing card's Depth of Field section (#1445): the mode, then the
//! running mode's knobs and its quality level (#1522), through the effect's knob table, the one the `Graphics.*`
//! functions and the tween paths are made from, so the three reach the same knobs.

use super::{quality_row, slider_row, Section};
use crate::components::{DepthOfField, DofMode};
use crate::core::settings::quality::{EffectQuality, MAX_RESOLUTION_DIVISOR};
use crate::scene::authoring::rendering::post_processing::{self as vc_ops, Knob};
use crate::scene::PostProcessingComponent;

pub(super) const SECTION: Section = Section {
    effect: "depth_of_field",
    group: None,
    order: 30,
    draw,
};

/// The mode selector and its knobs.
fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Depth of Field");
    let dof = vc.depth_of_field;
    let mut mode = dof.mode;
    ui.horizontal(|ui| {
        ui.label("  Mode:");
        egui::ComboBox::from_id_salt("dof_mode")
            .selected_text(mode.name())
            .show_ui(ui, |ui| {
                for value in DofMode::ALL {
                    if ui
                        .selectable_value(&mut mode, value, value.name())
                        .changed()
                    {
                        if let Some(mut c) = world.post_processing_mut(id) {
                            vc_ops::depth_of_field::set_mode(&mut c, mode);
                        }
                        *is_dirty = true;
                    }
                }
            });
    });
    let rows = match dof.mode {
        DofMode::Off => &[][..],
        DofMode::Gaussian => GAUSSIAN,
        DofMode::Bokeh => BOKEH,
    };
    for &(label, field, (lo, hi)) in rows {
        let knob = knob(field);
        let value = (knob.get)(vc);
        *is_dirty |= slider_row(ui, (&mut *world, id), label, value, lo..=hi, knob.set);
    }
    if dof.mode == DofMode::Off {
        return;
    }
    if let Some(quality) = quality_row(ui, "DoF Quality", dof.quality) {
        if let Some(mut c) = world.post_processing_mut(id) {
            vc_ops::depth_of_field::set_quality(&mut c, quality);
        }
        *is_dirty = true;
    }
    if dof.quality == EffectQuality::Custom {
        let knob = knob("resolution_divisor");
        let (label, range) = (
            "    Resolution Divisor:",
            1.0..=MAX_RESOLUTION_DIVISOR as f32,
        );
        *is_dirty |= slider_row(ui, (world, id), label, (knob.get)(vc), range, knob.set);
    }
}

/// The depth of field's knob at `field`.
fn knob(field: &str) -> &'static Knob {
    super::knob(vc_ops::depth_of_field::EFFECT.knobs, field)
}

/// One knob's row: its label, its knob's field and the slider's range.
type Row = (&'static str, &'static str, (f32, f32));

const GAUSSIAN: &[Row] = &[
    ("    Start:", "gaussian_start", (0.0, 100.0)),
    ("    End:", "gaussian_end", (0.0, 200.0)),
    (
        "    Max Radius:",
        "gaussian_max_radius",
        (DepthOfField::MIN_MAX_RADIUS, DepthOfField::MAX_MAX_RADIUS),
    ),
];

const BOKEH: &[Row] = &[
    (
        "    Focus Distance:",
        "focus_distance",
        (DepthOfField::MIN_FOCUS_DISTANCE, 50.0),
    ),
    (
        "    Aperture (f-stop):",
        "aperture",
        (DepthOfField::MIN_APERTURE, DepthOfField::MAX_APERTURE),
    ),
    (
        "    Focal Length (mm):",
        "focal_length",
        (
            DepthOfField::MIN_FOCAL_LENGTH,
            DepthOfField::MAX_FOCAL_LENGTH,
        ),
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_names_a_knob_and_every_knob_has_a_row() {
        let fields: Vec<_> = GAUSSIAN.iter().chain(BOKEH).map(|r| r.1).collect();
        for field in &fields {
            knob(field);
        }
        let all = vc_ops::depth_of_field::EFFECT.knobs.iter().map(|k| k.field);
        assert!(all.eq(fields.iter().copied()), "rows follow the knob table");
    }
}
