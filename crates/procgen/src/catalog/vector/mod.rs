//! crates/procgen/src/catalog/vector/mod.rs — the vector / normal ops, one file each.
//! Their math lives in [`crate::ops::vector`] and, for `tile` and `scatter`,
//! [`crate::ops::tile`] and [`crate::ops::scatter`].

// sorted: one module per vector op; none reads another
pub mod bump_to_normal;
pub mod combine_rgb;
pub mod directional_warp;
pub mod mapping;
pub mod normal_blend;
pub mod sample_at;
pub mod scatter;
pub mod separate_rgb;
pub mod tile;
pub mod warp;
