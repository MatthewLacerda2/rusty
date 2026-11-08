//! src/scene/lighting/lightmap/ — baked lightmaps for static geometry (#438).
//!
//! Static meshes with a second UV map (glTF `TEXCOORD_1`) get a per-texel lightmap:
//! bounce light, sky, emissive surfaces and the direct light of `Baked` lights, from
//! a CPU path tracer. Dynamic objects keep the light probes, so the two stay one
//! lighting. The pieces:
//!
//! * `input` — the static scene flattened for the bake ([`BakeScene::gather`]);
//! * `lights` — the lights it bakes, each one's [`LightShape`], in `lights::area`
//!   how an Area light's rectangle is sampled and falls off (#1674), and in
//!   `lights::channels` which shadowmask channel each `Mixed` light gets (#1695);
//! * `rays` / `raster` / `trace` — ray queries (parry's `TriMesh`), texel → surface
//!   point, and the per-texel estimate (the units it stores are explained in `trace`).
//!   `rays` and `raster` are shared with the mesh-map baker (`lighting::mesh_maps`,
//!   #1849), which walks a model's UV0 the same way;
//! * `filter` — the edge-aware Gaussian that smooths the baked bounce;
//! * `bake` — the parallel, seeded driver ([`bake`]), watched and stopped through
//!   `progress` ([`BakeProgress`], #808); its `pool` is the one work pool every
//!   mesh's texels share, in fixed-size batches (#1656), and the mesh-map baker
//!   runs its texels on too (`run_pool`, #1849);
//! * `atlas` — packing the lightmaps into equal-size pages ([`pack`]), each map in
//!   a cell its BC blocks and first [`MIP_LEVELS`] mips never leave (#1694);
//! * `encode` — RGBM, how a page's fallback is stored as an 8-bit PNG, the RGBA8 direction
//!   page a directional bake adds (#810) and the RGBA8 shadowmask page a
//!   `Shadowmask` bake adds (#1695);
//! * `pages` — a packed page's mip chain, as BC6H irradiance and a BC7 direction
//!   page (#1694), with the RGBM PNG beside it as the fallback;
//! * `set` — the scene's pages and each entity's place in them ([`LightmapSet`]);
//! * `probe_direct` — the direct light of `Baked` lights at light probes, which the
//!   probe bake adds analytically ([`baked_direct_sh`], #809), and the probes'
//!   shadowmask occlusion ([`mixed_occlusion`], #1695).
//!
//! The bake is a pure function of (scene, settings, seed): no clock, no unseeded RNG,
//! and the result never depends on how many threads ran it.

mod atlas;
mod bake;
mod encode;
mod filter;
mod input;
mod lights;
mod pages;
mod probe_direct;
mod progress;
mod raster;
mod rays;
mod rng;
mod set;
mod trace;

pub use atlas::{
    check_page_count, max_map_edge, pack, page_count, page_edge, LightmapAtlas, MAX_PAGE,
    MAX_PAGES, MIN_PAGE, MIP_LEVELS, PAD,
};
pub(in crate::scene::lighting) use bake::pool::run as run_pool;
pub use bake::{bake, bake_with_progress, BakeSettings, Lightmap};
pub use encode::{
    decode_direction, decode_mask, decode_rgbm, encode_directions, encode_masks, encode_texels,
};
pub use input::{BakeLight, BakeMesh, BakeRun, BakeScene, LightShape};
pub use lights::area::AreaShape;
pub use lights::{assign_channels, taken_lights, MaskChannels, MASK_CHANNELS};
pub use pages::{colour_page, direction_levels, direction_page, level_count, levels, mask_levels};
pub use probe_direct::{baked_direct_sh, mixed_occlusion};
pub use progress::BakeProgress;
pub use raster::{lightmap_size, MIN_RESOLUTION};
pub(in crate::scene::lighting) use raster::{rasterize_surface, world_per_uv, Surface, TexelPoint};
pub(in crate::scene::lighting) use rays::{Rays, Tri};
pub(in crate::scene::lighting) use rng::Rng;
pub use set::{LightmapEntry, LightmapSet};
#[cfg(all(test, feature = "dev"))]
pub(crate) use trace::bench::trace_texels;
pub(in crate::scene::lighting) use trace::cosine_sample;

#[cfg(test)]
mod ambient_tests;
#[cfg(test)]
mod area_reach_tests;
#[cfg(test)]
mod area_tests;
#[cfg(test)]
mod casting_tests;
#[cfg(test)]
mod coverage_tests;
#[cfg(test)]
mod direction_tests;
#[cfg(test)]
mod falloff_tests;
#[cfg(test)]
mod gather_tests;
#[cfg(test)]
mod inset_tests;
#[cfg(test)]
mod mask_tests;
#[cfg(test)]
mod pages_bc_tests;
#[cfg(test)]
mod pages_tests;
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
mod scale_tests;
#[cfg(test)]
mod share_bc7_tests;
#[cfg(test)]
mod share_tests;
#[cfg(test)]
mod skybox_tests;
#[cfg(test)]
mod sliver_tests;
#[cfg(test)]
mod tests;
