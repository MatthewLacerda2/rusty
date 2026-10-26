//! The colour grade (#1440, #1441): the effects the grading bake runs, in their
//! order, on each texel of the view's LUT, so the composite grades a pixel with one
//! fetch. White balance, Lift / Gamma / Gain and the hue shift act on the
//! scene-linear colour, before the tonemap; contrast, saturation and the display
//! gamma on the tonemapped colour; the Color Lookup last, on the display colour.
//!
//! Each packs from the view's volume, holding its knobs to the ops' ranges again
//! (a loaded file bypasses the ops, serde writing the fields directly), and packs the
//! neutral grade (no tonemap) without one.

// sorted: one module per grading effect; none reads another
pub(crate) mod color_adjustments;
pub(crate) mod color_lookup;
pub(crate) mod lift_gamma_gain;
pub(crate) mod tonemapping;
pub(crate) mod white_balance;

#[cfg(test)]
mod pack_tests;
#[cfg(test)]
mod prepare_tests;

/// `v` clamped to `-reach..=reach`, an infinity saturating; `0` (neutral) when NaN.
fn bounded(v: f32, reach: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(-reach, reach)
    }
}
