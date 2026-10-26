//! The colour grade of a post-processing volume (#1440, #1441): the effects the
//! renderer bakes, with the tonemapper, into the view's grading LUT, Unity URP's
//! overrides of the same names.

// sorted: one module per grading effect; none reads another
mod color_adjustments;
mod color_lookup;
mod lift_gamma_gain;
mod tonemapping;
mod white_balance;

// sorted: each effect's settings, re-exported by name
pub use color_adjustments::ColorAdjustments;
pub use color_lookup::ColorLookup;
pub use lift_gamma_gain::{ColorWheel, LiftGammaGain, Wheel};
pub use tonemapping::{Tonemap, Tonemapping};
pub use white_balance::WhiteBalance;
