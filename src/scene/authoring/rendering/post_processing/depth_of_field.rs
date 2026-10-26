//! The depth of field's ops (#1445): the mode and each mode's parameters, clamped
//! to the ranges Unity's override takes. [`DOF_KNOBS`] names the numeric ones once,
//! for the Inspector, the `Graphics.*` functions and the
//! `"Graphics.depth_of_field.<knob>"` tween paths alike: all three write through it,
//! so none can reach a knob the others can't.

use crate::components::{DepthOfField, DofMode, PostProcessingComponent};
use crate::scene::authoring::sanitize::{at_least, clamped, non_negative};

/// Set which depth of field runs.
pub fn set_dof_mode(vc: &mut PostProcessingComponent, mode: DofMode) {
    vc.depth_of_field.mode = mode;
}

/// Set where the Gaussian blur starts, `≥ 0` metres; the end follows it out.
pub fn set_dof_gaussian_start(vc: &mut PostProcessingComponent, metres: f32) {
    let dof = &mut vc.depth_of_field;
    dof.gaussian_start = non_negative(metres);
    dof.gaussian_end = dof.gaussian_end.max(dof.gaussian_start);
}

/// Set where the Gaussian blur is full, floored at its start.
pub fn set_dof_gaussian_end(vc: &mut PostProcessingComponent, metres: f32) {
    let dof = &mut vc.depth_of_field;
    dof.gaussian_end = at_least(metres, dof.gaussian_start);
}

/// Set the Gaussian blur's full radius, clamped to `0.5..=1.5`.
pub fn set_dof_gaussian_max_radius(vc: &mut PostProcessingComponent, radius: f32) {
    let (lo, hi) = (DepthOfField::MIN_MAX_RADIUS, DepthOfField::MAX_MAX_RADIUS);
    vc.depth_of_field.gaussian_max_radius = clamped(radius, lo, hi);
}

/// Set the Bokeh focus distance, at least `0.1` metres.
pub fn set_dof_focus_distance(vc: &mut PostProcessingComponent, metres: f32) {
    vc.depth_of_field.focus_distance = at_least(metres, DepthOfField::MIN_FOCUS_DISTANCE);
}

/// Set the Bokeh f-stop, clamped to `1..=32`.
pub fn set_dof_aperture(vc: &mut PostProcessingComponent, f_stop: f32) {
    let (lo, hi) = (DepthOfField::MIN_APERTURE, DepthOfField::MAX_APERTURE);
    vc.depth_of_field.aperture = clamped(f_stop, lo, hi);
}

/// Set the Bokeh focal length, clamped to `1..=300` millimetres.
pub fn set_dof_focal_length(vc: &mut PostProcessingComponent, mm: f32) {
    let (lo, hi) = (
        DepthOfField::MIN_FOCAL_LENGTH,
        DepthOfField::MAX_FOCAL_LENGTH,
    );
    vc.depth_of_field.focal_length = clamped(mm, lo, hi);
}

/// One numeric depth-of-field knob: its field name, the `Graphics.{Get,Set}<api>`
/// name, its reader and its op.
#[derive(Debug)]
pub struct DofKnob {
    /// The field, as the scene file and the tween path (`depth_of_field.<field>`)
    /// spell it.
    pub field: &'static str,
    /// The `Graphics.Get<api>` / `Set<api>` name.
    pub api: &'static str,
    pub get: fn(&DepthOfField) -> f32,
    pub set: fn(&mut PostProcessingComponent, f32),
}

/// Every numeric depth-of-field knob, in the inspector's order.
pub const DOF_KNOBS: &[DofKnob] = &[
    DofKnob {
        field: "gaussian_start",
        api: "DepthOfFieldGaussianStart",
        get: |d| d.gaussian_start,
        set: set_dof_gaussian_start,
    },
    DofKnob {
        field: "gaussian_end",
        api: "DepthOfFieldGaussianEnd",
        get: |d| d.gaussian_end,
        set: set_dof_gaussian_end,
    },
    DofKnob {
        field: "gaussian_max_radius",
        api: "DepthOfFieldGaussianMaxRadius",
        get: |d| d.gaussian_max_radius,
        set: set_dof_gaussian_max_radius,
    },
    DofKnob {
        field: "focus_distance",
        api: "DepthOfFieldFocusDistance",
        get: |d| d.focus_distance,
        set: set_dof_focus_distance,
    },
    DofKnob {
        field: "aperture",
        api: "DepthOfFieldAperture",
        get: |d| d.aperture,
        set: set_dof_aperture,
    },
    DofKnob {
        field: "focal_length",
        api: "DepthOfFieldFocalLength",
        get: |d| d.focal_length,
        set: set_dof_focal_length,
    },
];

/// The knob a tween path's name (after `Graphics.`) names, `depth_of_field.<field>`.
pub fn dof_knob(name: &str) -> Option<&'static DofKnob> {
    let field = name.strip_prefix("depth_of_field.")?;
    DOF_KNOBS.iter().find(|k| k.field == field)
}

impl DofKnob {
    /// The knob of `field`. Panics on a field [`DOF_KNOBS`] lacks: a caller names
    /// one of its own.
    pub fn of(field: &str) -> &'static DofKnob {
        let knob = DOF_KNOBS.iter().find(|k| k.field == field);
        knob.unwrap_or_else(|| panic!("no depth-of-field knob `{field}`"))
    }
}
