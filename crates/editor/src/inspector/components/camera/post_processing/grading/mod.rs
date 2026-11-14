//! The Color Correction sections of the Post Processing card (#1440, #1441), one
//! per grading effect, all under the Color Correction heading.

// sorted: one module per grading section; none reads another
pub(super) mod color_adjustments;
pub(super) mod color_lookup;
pub(super) mod lift_gamma_gain;
pub(super) mod tonemapping;
pub(super) mod white_balance;
