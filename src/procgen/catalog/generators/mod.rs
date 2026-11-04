//! src/procgen/catalog/generators/mod.rs — the source ops (no inputs), one file
//! each. Their samplers live in [`crate::procgen::ops::generators`], which says why
//! each one tiles.
//!
//! Periodic params (`scale`, `frequency`, `rows`/`cols`, `tiles`) are whole counts
//! of periods per tile: fractional values round at evaluation (#392). A stochastic
//! op's `seed` (default 0, [`super::super::recipe::SEED`]) is folded into the
//! recipe seed (`hash::node_seed`), so two otherwise identical nodes differ by seed
//! alone (#1487); 0 keeps the recipe seed.

// sorted: one module per generator op; none reads another
pub mod anisotropic_noise;
pub mod brick;
pub mod checker;
pub mod constant;
pub mod gradient;
pub mod image;
pub mod noise;
pub mod shape;
pub mod voronoi;
pub mod wave;
pub mod white_noise;
