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

fn bounded(v: f32, reach: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(-reach, reach)
    }
}
