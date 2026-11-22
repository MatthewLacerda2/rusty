//! The colour ops, one file each. Their math lives in [`crate::ops::color`], or in the op's own
//! file when small (`histogram_scan`, `levels`): pixel to pixel, unclamped linear float.
// sorted: one module per colour op; none reads another
pub mod auto_levels;
pub mod bright_contrast;
pub mod color_ramp;
#[cfg(test)]
pub mod dummy;
pub mod gamma;
pub mod histogram_scan;
pub mod hue_sat_value;
pub mod invert;
pub mod levels;
pub mod mix;
