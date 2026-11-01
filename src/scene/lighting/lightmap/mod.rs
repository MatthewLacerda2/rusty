//! src/scene/lighting/lightmap/ — baked lightmaps for static geometry (#438).
//!
//! Static meshes with a second UV map (glTF `TEXCOORD_1`) get a per-texel lightmap:
//! bounce light, sky, emissive surfaces and the direct light of `Baked` lights, from
//! a CPU path tracer. Dynamic objects keep the light probes, so the two stay one
//! lighting. The pieces:
//!
//! * `input` — the static scene flattened for the bake ([`BakeScene::gather`]);
//! * `lights` — the lights it bakes, each one's [`LightShape`], and in `lights::area`
//!   how an Area light's rectangle is sampled and falls off (#1674);
//! * `rays` / `raster` / `trace` — ray queries (parry's `TriMesh`), texel → surface
//!   point, and the per-texel estimate (the units it stores are explained in `trace`);
//! * `filter` — the edge-aware Gaussian that smooths the baked bounce;
//! * `bake` — the parallel, seeded driver ([`bake`]), watched and stopped through
//!   `progress` ([`BakeProgress`], #808); its `pool` is the one work pool every
//!   mesh's texels share, in fixed-size batches (#1656);
//! * `atlas` — packing the lightmaps into equal-size pages ([`pack`]);
//! * `encode` — RGBM, how a page is stored as an 8-bit PNG, and the RGBA8 direction
//!   page a directional bake adds (#810);
//! * `set` — the scene's pages and each entity's place in them ([`LightmapSet`]);
//! * `probe_direct` — the direct light of `Baked` lights at light probes, which the
//!   probe bake adds analytically ([`baked_direct_sh`], #809).
//!
//! The bake is a pure function of (scene, settings, seed): no clock, no unseeded RNG,
//! and the result never depends on how many threads ran it.

mod atlas;
mod bake;
mod encode;
mod filter;
mod input;
mod lights;
mod probe_direct;
mod progress;
mod raster;
mod rays;
mod rng;
mod set;
mod trace;

pub use atlas::{pack, LightmapAtlas, MAX_PAGE};
pub use bake::{bake, bake_with_progress, BakeSettings, Lightmap};
pub use encode::{encode_directions, encode_texels};
pub use input::{BakeLight, BakeMesh, BakeRun, BakeScene, LightShape};
pub use lights::area::AreaShape;
pub use probe_direct::baked_direct_sh;
pub use progress::BakeProgress;
pub use raster::{lightmap_size, MIN_RESOLUTION};
pub use set::{LightmapEntry, LightmapSet};

#[cfg(test)]
mod ambient_tests;
#[cfg(test)]
mod area_reach_tests;
#[cfg(test)]
mod area_tests;
#[cfg(test)]
mod atlas_tests;
#[cfg(test)]
mod casting_tests;
#[cfg(test)]
mod direction_tests;
#[cfg(test)]
mod gather_tests;
#[cfg(test)]
mod pool_tests;
#[cfg(test)]
mod probe_direct_tests;
#[cfg(test)]
mod progress_tests;
#[cfg(test)]
mod rays_tests;
#[cfg(test)]
mod run_bake_tests;
#[cfg(test)]
mod run_tests;
#[cfg(test)]
mod sliver_tests;
#[cfg(test)]
mod tests;
