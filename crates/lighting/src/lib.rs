//! crates/lighting/src/lib.rs — the lighting bakes: what Generate Lighting computes
//! from a scene, kept apart from the data it writes (`rusty_scene::lighting`) so an
//! edit to a bake rebuilds only the crates that run one.
//!
//!   lightmap  — the CPU path-traced lightmapper, its atlas and pages (#438)
//!   occlusion — the occlusion-culling PVS bake (#1714)
//!   placement — where light and reflection probes go (#246, #1737)
//!   mesh_maps — a model's baked mesh maps: AO, curvature, edges (#1849)
//!
//! Every bake is a pure function of (scene, settings, seed): no clock, no unseeded
//! RNG, and the bytes never depend on how many threads ran it.

// Panic-free sim (#195, #962): bare `.unwrap()` is denied in every sim module (use `?`
// or a documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod lightmap;
pub mod mesh_maps;
pub mod occlusion;
pub mod placement;
