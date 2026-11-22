//! The converter / math ops, one file each. Their math lives in [`crate::ops::math`]: per channel,
//! pixel to pixel; `expression`'s formula language in [`crate::ops::expression`].
// sorted: one module per math op; none reads another
// `math` is the `arithmetic` file: a module may not share its parent's name.
pub mod arithmetic;
pub mod clamp;
pub mod expression;
pub mod map_range;
pub mod rgb_to_bw;
