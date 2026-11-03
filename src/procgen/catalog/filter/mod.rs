//! src/procgen/catalog/filter/mod.rs — the neighbourhood ops, one file each. Their
//! math lives in [`crate::procgen::ops::filter`] and, for `distance`,
//! [`crate::procgen::ops::distance`]; every reach is in tile units, so a filter
//! looks the same at any resolution.

// sorted: one module per filter op; none reads another
pub mod blur;
pub mod cavity;
pub mod curvature;
pub mod distance;
