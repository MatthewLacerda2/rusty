//! `Graphics.*` for the colour grade (#1440, #1441), one file per grading effect.
//! The renderer bakes them into the view's grading LUT.

// sorted: one module per grading effect; none reads another
pub(super) mod color_adjustments;
pub(super) mod color_lookup;
pub(super) mod lift_gamma_gain;
pub(super) mod tonemapping;
pub(super) mod white_balance;
