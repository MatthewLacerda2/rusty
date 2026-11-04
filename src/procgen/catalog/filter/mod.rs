//! src/procgen/catalog/filter/mod.rs — the neighbourhood ops, one file each,
//! clustered by what they read (#1751), so the family stays under the `dir-size`
//! cap as it grows. Their math lives in [`crate::procgen::ops::filter`] and, for
//! `distance` and `edge_detect`, [`crate::procgen::ops::distance`] (`highpass` and
//! `sharpen` difference `ops::filter`'s blur; `ambient_occlusion` keeps its own); every reach is in tile units, so
//! a filter looks the same at any resolution.
//!
//! - [`blurs`] — a kernel that smears an image, or the difference from one: `blur`,
//!   `highpass`, `sharpen` (planned: `directional_blur`, `slope_blur`);
//! - [`mask`] — a mask's shape: `distance`, `edge_detect`;
//! - [`relief`] — a height map in, a mask out: `cavity`, `curvature`,
//!   `ambient_occlusion`.
//!
//! The flood-fill ops (`flood_fill` and its readers) start a `flood/` cluster of
//! their own.

// sorted: one module per cluster; none reads another
pub mod blurs;
pub mod mask;
pub mod relief;
#[cfg(test)]
pub(crate) mod test_probe;
