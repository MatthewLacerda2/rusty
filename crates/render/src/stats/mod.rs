#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderCounters {
    pub draw_calls: u32,
    pub triangles: u64,
    pub visible_entities: u32,
    pub cull_tested: u32,
    pub culled_entities: u32,
    pub size_culled_entities: u32,
    pub shadow_cull_tested: u32,
    pub shadow_frustum_culled: u32,
    pub shadow_size_culled: u32,
    pub shadow_occlusion_culled: u32,
    pub shadow_gather_us: u64,
    pub occlusion_culled: u32,
    pub hiz_tested: u32,
    pub hiz_culled: u32,
    pub hiz_late: u32,
    pub hiz_depth_triangles: u64,
    pub cluster_draws: u32,
    pub cluster_tested: u32,
    pub cluster_culled: u32,
    pub lod_hidden_entities: u32,
    pub lights: u32,
    pub lights_dropped: u32,
    pub cluster_lights_dropped: u32,
    pub lights_visible: u32,
    pub lights_culled: u32,
    pub light_cluster_refs: u64,
    pub light_bin_us: u64,
    pub decals_visible: u32,
    pub decal_cluster_refs: u64,
    pub decal_maps_dropped: u32,
    pub shadow_draws: u32,
    pub shadowed_lights: u32,
    pub shadow_lights_dropped: u32,
    pub shadow_atlas_tiles: u32,
    pub shadow_atlas_texels: u64,
    pub shadow_atlas_cached: u32,
    pub shadow_atlas_rebaked: u32,
    pub ui_draws: u32,
    pub ui_meshed: u32,
    pub ui_laid_out: u32,
    pub ui_relaid: [u32; 7],
    pub ui_mask_passes: u32,
    pub ui_blur_passes: u32,
    pub ssao_samples: u64,
    pub ssil_samples: u64,
    pub ssr_ray_texels: u64,
    pub volumetric_froxels: u64,
    pub particles_drawn: u32,
    pub render_texture_draws: u32,
    pub velocity_passes: u32,
    pub primed_passes: u32,
    pub forward_lit_primed: u32,
    pub forward_lit_unprimed: u32,
    pub forward_unlit: u32,
    pub history_passes: u32,
    pub ssr_surface_passes: u32,
    pub grade_lut_bakes: u32,
    pub sky_reflection_bakes: u32,
    pub opaque_copies: u32,
    pub texture_bytes: u64,
    pub lightmap_bytes: u64,
    pub reflection_bytes: u64,
    pub mesh_count: u32,
    pub mesh_bytes: u64,
    pub cluster_pool_bytes: u64,
    pub ray_tree_bytes: u64,
    pub ray_tree_rebuilds: u32,
    pub ray_mesh_builds: u32,
    pub render_extracted: u32,
    pub render_posed: u32,
    pub render_extract_us: u64,
    pub render_upload_bytes: u64,
    pub uploads: Uploads,
    pub ops: OpCounts,
}

mod frame;
mod memory;
mod pairs;
pub(crate) mod switches;
mod uploads;
pub use crate::gpu::ops::OpCounts;
pub use memory::{gpu_memory, GpuMemory};
pub use switches::{EarlyFlushes, RenderSwitch, RenderSwitches};
pub use uploads::Uploads;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaleOcclusion {
    pub changed: usize,
    pub occluders: usize,
}

#[cfg(test)]
use frame::{count_lights, draws_of};

#[cfg(test)]
mod forward_gpu_tests;
#[cfg(test)]
mod ops_gpu_tests;
#[cfg(test)]
#[path = "stats_tests.rs"]
mod stats_tests;
