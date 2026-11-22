//! Baked lightmaps for static meshes with a second UV: a CPU path tracer bakes bounce, sky, emission and `Baked` lights' direct light per texel; dynamic objects keep the light probes. `rays` and `raster` are shared with `lighting::mesh_maps`.
//!
//! The bake is a pure function of (scene, settings, seed): no clock, no unseeded RNG, any thread count.

mod atlas;
mod bake;
use rusty_scene::lighting::page::encode;
mod filter;
mod input;
mod lights;
mod pages;
mod probe_direct;
mod progress;
mod raster;
mod rays;
mod rng;
mod trace;

pub use atlas::{
    check_page_count, max_map_edge, pack, packing, page_count, AtlasPage, LightmapAtlas, Packing,
    MAX_PAGES, PAD,
};
pub(crate) use bake::pool::run as run_pool;
// A test hook: `dev::bake::lightmap`'s memory test bakes on a fixed number of workers.
#[doc(hidden)]
pub use bake::bake_on;
pub use bake::{
    bake, bake_progressive, bake_with_progress, BakeView, Lightmap, LightmapDenoiser, Pass,
    PassSink, ASSEMBLE_ALONE,
};
pub use encode::{
    decode_direction, decode_mask, decode_rgbm, encode_directions, encode_masks, encode_texels,
};
pub use input::{BakeLight, BakeMesh, BakeRun, BakeScene, LightShape, MapAverage, MapEncoding};
pub use lights::area::AreaShape;
pub use lights::{assign_channels, taken_lights, MaskChannels, TakenLights, MASK_CHANNELS};
pub use pages::{colour_page, direction_levels, direction_page, level_count, levels, mask_levels};
pub use probe_direct::{baked_direct_sh, mixed_occlusion, probe_direct};
pub use progress::{BakeObserver, BakeProgress};
pub use raster::{lightmap_size, MIN_RESOLUTION};
pub(crate) use raster::{rasterize_claims, world_per_uv, Areas, Claim, Surface, TexelPoint};
pub(crate) use rays::{Rays, Tri};
pub(crate) use rng::Rng;
#[cfg(feature = "dev")]
#[doc(hidden)]
pub use trace::bench::{trace_texels, TraceCost};
pub(crate) use trace::cosine_sample;
// The knobs the bake reads, in the scope of the tests' `use super::*`.
#[cfg(test)]
use rusty_scene::lighting::settings::{BakeSettings, Denoiser, MAX_PAGE};

#[cfg(test)]
mod ambient_tests;
#[cfg(test)]
mod area_reach_tests;
#[cfg(test)]
mod area_tests;
#[cfg(test)]
mod buried_tests;
#[cfg(test)]
mod casting_tests;
#[cfg(test)]
mod coverage_tests;
#[cfg(test)]
mod denoise_tests;
#[cfg(test)]
mod direction_tests;
#[cfg(test)]
mod falloff_tests;
#[cfg(test)]
mod gather_tests;
#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod inset_tests;
#[cfg(test)]
mod mask_tests;
#[cfg(test)]
mod occlusion_map_tests;
#[cfg(test)]
mod pages_bc_tests;
#[cfg(test)]
mod pages_tests;
#[cfg(test)]
mod pass_tests;
#[cfg(test)]
mod pool_made_tests;
#[cfg(test)]
mod pool_tests;
#[cfg(test)]
mod prioritize_tests;
#[cfg(test)]
mod probe_check_tests;
#[cfg(test)]
mod probe_direct_tests;
#[cfg(test)]
mod progress_tests;
#[cfg(test)]
mod progressive_tests;
#[cfg(test)]
mod run_bake_tests;
#[cfg(test)]
mod run_tests;
#[cfg(test)]
mod scale_tests;
#[cfg(test)]
mod shadow_probe_tests;
#[cfg(test)]
mod share_tests;
#[cfg(test)]
mod skybox_tests;
#[cfg(test)]
mod slit_probe_tests;
#[cfg(test)]
mod sliver_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod validity_tests;
#[cfg(test)]
mod view_tests;
