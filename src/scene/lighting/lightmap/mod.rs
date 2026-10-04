//! src/scene/lighting/lightmap/ — baked lightmaps for static geometry (#438).
//!
//! Static meshes with a second UV map (glTF `TEXCOORD_1`) get a per-texel lightmap:
//! bounce light, sky, emissive surfaces and the direct light of `Baked` lights, from
//! a CPU path tracer. Dynamic objects keep the light probes, so the two stay one
//! lighting. The pieces:
//!
//! * `input` — the static scene flattened for the bake ([`BakeScene::gather`]);
//! * `bvh` / `raster` / `trace` — ray acceleration, texel → surface point, and the
//!   per-texel estimate (the units it stores are explained in `trace`);
//! * `filter` — the edge-aware Gaussian that smooths the baked bounce;
//! * `bake` — the parallel, seeded driver ([`bake`]);
//! * `atlas` — packing the lightmaps into equal-size pages ([`pack`]);
//! * `encode` — RGBM, how a page is stored as an 8-bit PNG;
//! * `set` — the scene's pages and each entity's place in them ([`LightmapSet`]).
//!
//! The bake is a pure function of (scene, settings, seed): no clock, no unseeded RNG,
//! and the result never depends on how many threads ran it.

mod atlas;
mod bake;
mod bvh;
mod encode;
mod filter;
mod input;
mod raster;
mod rng;
mod set;
mod trace;

pub use atlas::{pack, LightmapAtlas, MAX_PAGE};
pub use bake::{bake, BakeSettings, Lightmap};
pub use encode::{decode_rgbm, encode_rgbm, encode_texels, RGBM_RANGE};
pub use input::{BakeLight, BakeMesh, BakeScene, LightShape};
pub use raster::{lightmap_size, MIN_RESOLUTION};
pub use set::{LightmapEntry, LightmapSet};

#[cfg(test)]
mod atlas_tests;
#[cfg(test)]
mod gather_tests;
#[cfg(test)]
mod tests;
