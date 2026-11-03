//! src/procgen/catalog/math/mod.rs — the converter / math ops, one file each.
//! Their math lives in [`crate::procgen::ops::math`]: per channel, pixel to pixel.

// sorted: one module per math op; none reads another
pub mod clamp;
pub mod map_range;
pub mod math;
pub mod rgb_to_bw;
