//! crates/procgen/src/catalog/filter/relief/mod.rs — the filters that read the input's red
//! as a height and write a mask from its relief: crevices, edges, occlusion.

// sorted: one module per op; none reads another
pub mod ambient_occlusion;
pub mod cavity;
pub mod curvature;
