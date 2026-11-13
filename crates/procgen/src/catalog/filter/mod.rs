//! crates/procgen/src/catalog/filter/mod.rs — the neighbourhood ops, one file each,
//! clustered by what they read (#1751), so the family stays under the `dir-size`
//! cap as it grows. Their math lives in [`crate::ops::filter`] and, for
//! `distance` and `edge_detect`, [`crate::ops::distance`] (`highpass` and
//! `sharpen` difference `ops::filter`'s blur; `ambient_occlusion` keeps its own;
//! the `flood_*` ops label islands in [`crate::ops::flood`]); every reach
//! is in tile units, so a filter looks the same at any resolution.
//!
//! - [`blurs`] — a kernel that smears an image, or the difference from one: `blur`,
//!   `highpass`, `sharpen` (planned: `directional_blur`, `slope_blur`);
//! - [`flood`] — a mask's islands, and a value per island: `flood_fill`,
//!   `flood_random`, `flood_gradient`, `flood_bbox`;
//! - [`mask`] — a mask's shape: `distance`, `edge_detect`;
//! - [`relief`] — a height map in, a mask out: `cavity`, `curvature`,
//!   `ambient_occlusion`.

// sorted: one module per cluster; none reads another
pub mod blurs;
pub mod flood;
pub mod mask;
pub mod relief;
#[cfg(test)]
pub(crate) mod test_probe;
