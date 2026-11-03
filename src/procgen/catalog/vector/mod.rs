//! src/procgen/catalog/vector/mod.rs — the vector / normal ops, one file each.
//! Their math lives in [`crate::procgen::ops::vector`] and, for `tile`,
//! [`crate::procgen::ops::tile`].

// sorted: one module per vector op; none reads another
pub mod bump_to_normal;
pub mod combine_rgb;
pub mod mapping;
pub mod normal_blend;
pub mod separate_rgb;
pub mod tile;
pub mod warp;
