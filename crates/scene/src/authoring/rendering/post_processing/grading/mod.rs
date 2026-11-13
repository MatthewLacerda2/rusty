//! The colour grade's ops (#1440, #1441): the effects the renderer bakes into the
//! view's grading LUT. Each clamps its knobs to the ranges Unity's overrides take.

// sorted: one module per grading effect; none reads another
pub mod color_adjustments;
pub mod color_lookup;
pub mod lift_gamma_gain;
pub mod tonemapping;
pub mod white_balance;
