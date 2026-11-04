//! src/procgen/catalog/color/mod.rs — the colour ops, one file each. Their math
//! lives in [`crate::procgen::ops::color`]: pixel to pixel, unclamped linear float.

// sorted: one module per colour op; none reads another
pub mod auto_levels;
pub mod bright_contrast;
pub mod color_ramp;
#[cfg(test)]
pub mod dummy;
pub mod gamma;
pub mod hue_sat_value;
pub mod invert;
pub mod mix;
