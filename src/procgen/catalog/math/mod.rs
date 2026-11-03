//! src/procgen/catalog/math/mod.rs — the converter / math ops, one file each.
//! Their math lives in [`crate::procgen::ops::math`]: per channel, pixel to pixel.

// sorted: one module per math op; none reads another
// `math` is the `arithmetic` file: a module may not share its parent's name.
pub mod arithmetic;
pub mod clamp;
pub mod map_range;
pub mod rgb_to_bw;
