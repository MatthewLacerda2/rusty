//! The Post Processing card's Depth of Field section (#1445): the mode, then the
//! running mode's knobs. Split out of `post_processing` to keep it under the size
//! cap.
//!
//! A THIN client (#287): each widget reads the snapshot and writes through the
//! shared `authoring::rendering::post_processing` op `Graphics.*` calls too; the
//! knobs through `DOF_KNOBS`, the table the `Graphics.*` functions and the tween
//! paths are made from, so the three reach the same knobs.

use super::post_processing::slider_row;
use crate::components::{DepthOfField, DofMode};
use crate::scene::authoring::rendering::post_processing::{self as vc_ops, Knob};
use crate::scene::PostProcessingComponent;

/// The mode selector and its knobs.
pub(super) fn draw_depth_of_field(
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
}

/// The depth of field's knob at `field`, one this card names.
fn knob(field: &str) -> &'static Knob {
    let knob = vc_ops::depth_of_field::EFFECT.knob(field);
    knob.unwrap_or_else(|| panic!("no depth-of-field knob `{field}`"))
}

/// One knob's row: its label, its `DOF_KNOBS` field and the slider's range.
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
        assert!(all.eq(fields.iter().copied()), "rows follow DOF_KNOBS");
    }
}
