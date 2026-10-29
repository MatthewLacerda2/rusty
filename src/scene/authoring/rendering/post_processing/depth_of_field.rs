//! The depth of field's ops (#1445): the mode and each mode's parameters, clamped
//! to the ranges Unity's override takes. Its knobs are every numeric parameter.

use super::{Effect, Knob};
use crate::components::{DepthOfField, DofMode, PostProcessingComponent};
use crate::core::settings::quality::{EffectQuality, MAX_RESOLUTION_DIVISOR};
use crate::scene::authoring::fields::table::effect;
use crate::scene::authoring::sanitize::{clamped, count, non_negative, saturated};

/// Set which depth of field runs.
pub fn set_mode(vc: &mut PostProcessingComponent, mode: DofMode) {
    vc.depth_of_field.mode = mode;
}

/// Set where the Gaussian blur starts, `≥ 0` metres. Start and end are stored apart,
/// as Unity's are: an end left nearer than the start is drawn at the start (an
/// instant edge), and a tween of the start out and back leaves the authored end alone.
pub fn set_gaussian_start(vc: &mut PostProcessingComponent, metres: f32) {
    vc.depth_of_field.gaussian_start = non_negative(metres);
}

/// Set where the Gaussian blur is full, `≥ 0` metres, independent of the start.
pub fn set_gaussian_end(vc: &mut PostProcessingComponent, metres: f32) {
    vc.depth_of_field.gaussian_end = non_negative(metres);
}

/// Set the Gaussian blur's full radius, clamped to `0.5..=1.5`.
pub fn set_gaussian_max_radius(vc: &mut PostProcessingComponent, radius: f32) {
    let (lo, hi) = (DepthOfField::MIN_MAX_RADIUS, DepthOfField::MAX_MAX_RADIUS);
    vc.depth_of_field.gaussian_max_radius = clamped(radius, lo, hi);
}

/// Set the Bokeh focus distance, clamped to `0.1..=10000` metres; `math.huge` is
/// focus at infinity, the farthest.
pub fn set_focus_distance(vc: &mut PostProcessingComponent, metres: f32) {
    let (lo, hi) = (
        DepthOfField::MIN_FOCUS_DISTANCE,
        DepthOfField::MAX_FOCUS_DISTANCE,
    );
    vc.depth_of_field.focus_distance = saturated(metres, lo, hi);
}

/// Set the Bokeh f-stop, clamped to `1..=32`.
pub fn set_aperture(vc: &mut PostProcessingComponent, f_stop: f32) {
    let (lo, hi) = (DepthOfField::MIN_APERTURE, DepthOfField::MAX_APERTURE);
    vc.depth_of_field.aperture = clamped(f_stop, lo, hi);
}

/// Set the Bokeh focal length, clamped to `1..=300` millimetres.
pub fn set_focal_length(vc: &mut PostProcessingComponent, mm: f32) {
    let (lo, hi) = (
        DepthOfField::MIN_FOCAL_LENGTH,
        DepthOfField::MAX_FOCAL_LENGTH,
    );
    vc.depth_of_field.focal_length = clamped(mm, lo, hi);
}

/// Set which working resolution the blur runs at: a level of the active quality
/// preset, or `Custom`.
pub fn set_quality(vc: &mut PostProcessingComponent, quality: EffectQuality) {
    vc.depth_of_field.quality = quality;
}

/// Set the `Custom` working resolution's divisor of the view's size, clamped to
/// `1..=4`.
pub fn set_resolution_divisor(vc: &mut PostProcessingComponent, divisor: u32) {
    vc.depth_of_field.resolution_divisor = divisor.clamp(1, MAX_RESOLUTION_DIVISOR);
}

pub const EFFECT: Effect = Effect {
    name: "depth_of_field",
    field: effect!(depth_of_field, |c, v: DepthOfField| {
        set_mode(c, v.mode);
        set_gaussian_start(c, v.gaussian_start);
        set_gaussian_end(c, v.gaussian_end);
        set_gaussian_max_radius(c, v.gaussian_max_radius);
        set_focus_distance(c, v.focus_distance);
        set_aperture(c, v.aperture);
        set_focal_length(c, v.focal_length);
        set_quality(c, v.quality);
        set_resolution_divisor(c, v.resolution_divisor);
    }),
    knobs: &[
        Knob {
            field: "gaussian_start",
            api: Some("DepthOfFieldGaussianStart"),
            get: |vc| vc.depth_of_field.gaussian_start,
            set: set_gaussian_start,
        },
        Knob {
            field: "gaussian_end",
            api: Some("DepthOfFieldGaussianEnd"),
            get: |vc| vc.depth_of_field.gaussian_end,
            set: set_gaussian_end,
        },
        Knob {
            field: "gaussian_max_radius",
            api: Some("DepthOfFieldGaussianMaxRadius"),
            get: |vc| vc.depth_of_field.gaussian_max_radius,
            set: set_gaussian_max_radius,
        },
        Knob {
            field: "focus_distance",
            api: Some("DepthOfFieldFocusDistance"),
            get: |vc| vc.depth_of_field.focus_distance,
            set: set_focus_distance,
        },
        Knob {
            field: "aperture",
            api: Some("DepthOfFieldAperture"),
            get: |vc| vc.depth_of_field.aperture,
            set: set_aperture,
        },
        Knob {
            field: "focal_length",
            api: Some("DepthOfFieldFocalLength"),
            get: |vc| vc.depth_of_field.focal_length,
            set: set_focal_length,
        },
        Knob {
            field: "resolution_divisor",
            api: Some("DepthOfFieldResolutionDivisor"),
            get: |vc| vc.depth_of_field.resolution_divisor as f32,
            set: |vc, v| set_resolution_divisor(vc, count(v, 1, MAX_RESOLUTION_DIVISOR)),
        },
    ],
};
