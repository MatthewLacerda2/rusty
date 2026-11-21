//! crates/render/src/stats/ — what one rendered frame submitted (#433).
//!
//! `Renderer::render` fills a [`RenderCounters`] per frame so an agent can measure a
//! rendering change (instancing, LODs, clustered lights) instead of guessing. The
//! counts are taken from the draw lists the frame actually built — cheap, and exact
//! for everything that issues geometry. The dev layer folds them into `FrameStats`.

/// Counters for one `Renderer::render` call, summed over every camera in the stack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderCounters {
    /// Geometry draw calls: solids, transparents, shadow casters, the SSAO depth
    /// prepass, particle batches and UI batches; one mesh + material is one instanced
    /// draw (#470). Decals (folded into surfaces, #638), post-FX and the sky: none.
    pub draw_calls: u32,
    /// Triangles of the solid, transparent, shadow and prepass draws (not the GPU cull's late set).
    pub triangles: u64,
    /// Mesh entities drawn, summed over the camera stack.
    pub visible_entities: u32,
    /// Mesh entities tested against a camera's frustum (#1714): every one a camera
    /// draws at all whose LOD level it shows, summed over the camera views (the
    /// stack and render-texture cameras).
    pub cull_tested: u32,
    /// Mesh entities the frustum cull skipped, summed over the camera views.
    pub culled_entities: u32,
    /// Mesh entities inside a camera's frustum but past their max draw distance or
    /// under their min screen size (#1714), summed over the camera views.
    pub size_culled_entities: u32,
    /// Shadow casters tested against a light volume (a cascade or an atlas tile,
    /// #1714), one per entity per volume, summed over every shadow view the frame
    /// drew: its cached bakes, when they re-baked, and its per-frame sweeps.
    pub shadow_cull_tested: u32,
    /// Shadow casters outside a light volume, summed over the shadow views.
    pub shadow_frustum_culled: u32,
    /// Shadow casters inside a light volume but too far or too small from the base
    /// camera (#1714), summed over the shadow views.
    pub shadow_size_culled: u32,
    /// Static shadow casters inside a point or spot light's volume and kept by the
    /// size cull, but out of sight of the light's cell by the scene's baked PVS
    /// (#1714), summed over the shadow views.
    pub shadow_occlusion_culled: u32,
    /// CPU microseconds the shadow sweeps took to gather, cull and batch their
    /// casters (#1714), summed over the sweeps, inside `renderer_ms`. Wall-clock.
    pub shadow_gather_us: u64,
    /// Static mesh entities inside a camera's frustum and big enough, but out of
    /// sight of its cell by the scene's baked PVS (#1714), summed over the camera
    /// views. `0` with no bake, or for a camera with `use_occlusion_culling` off.
    pub occlusion_culled: u32,
    /// Opaque draw instances tested against a camera's Hi-Z depth pyramid (#1963):
    /// every one whose bounds are known, summed over the camera views.
    pub hiz_tested: u32,
    /// Opaque draw instances the depth pyramid found hidden (#1963), summed over
    /// the camera views. Read back from the GPU without waiting, so it describes
    /// a frame or so before this one, and `0` on the first.
    pub hiz_culled: u32,
    /// Of the visible ones, those phase one did not draw (#2336, the late set the
    /// prepass adds), summed over the views: `0` held still. Read back likewise.
    pub hiz_late: u32,
    /// Clusters of big meshes tested one by one after the Hi-Z instance test
    /// (#1968): every cluster of each instance it judged in a batch drawn by
    /// cluster, summed over the camera views.
    pub cluster_tested: u32,
    /// Of those, the ones left undrawn (behind the pyramid, out of the frustum,
    /// facing away, or of an instance found hidden). Read back like `hiz_culled`.
    pub cluster_culled: u32,
    /// Mesh entities hidden because their LODGroup showed another level (#472),
    /// summed over the camera stack.
    pub lod_hidden_entities: u32,
    /// Active lights in the scene.
    pub lights: u32,
    /// Lights left unlit: realtime directional lights past the 4 slots, every
    /// ambient light but the last, and per camera the point/spot lights in view
    /// past the clustered budget (64, the least contributing dropped; #434, #834).
    /// Summed over the stack.
    pub lights_dropped: u32,
    /// (cluster, light) entries cut because more point/spot lights reached one
    /// cluster than its budget of 32 (the least contributing there dropped; #834):
    /// non-zero means some corner of the view is over budget. Summed over the stack.
    pub cluster_lights_dropped: u32,
    /// Point/spot lights binned into at least one light cluster (#434), summed
    /// over the camera stack.
    pub lights_visible: u32,
    /// Point/spot lights listed in no cluster: outside a camera's view (#434), or
    /// kept but contributing nothing or with a cone missing the view (#1712), so
    /// no pixel shades them; summed over the camera stack.
    pub lights_culled: u32,
    /// Entries written to the cluster light lists (#434): how many (cluster,
    /// light) pairs the shaders may visit, summed over the camera stack.
    pub light_cluster_refs: u64,
    /// CPU microseconds spent binning lights and decals into clusters (#434,
    /// #638), summed over the camera stack. Wall-clock, so the one counter that
    /// varies run to run.
    pub light_bin_us: u64,
    /// Decals binned into at least one cluster (#638), summed over the camera stack.
    /// A stacked `DepthOnly` camera bins none.
    pub decals_visible: u32,
    /// Entries written to the cluster decal lists (#638): the (cluster, decal)
    /// pairs the shaders may visit, summed over the camera stack.
    pub decal_cluster_refs: u64,
    /// Decal maps drawn without their texture because the decal atlas was full
    /// (#638): more distinct maps in one frame than it has layers.
    pub decal_maps_dropped: u32,
    /// Shadow-caster draw calls (cascades' static bake + dynamic, and the point/spot
    /// shadow atlas); an instanced run of casters sharing a mesh is one draw (#470).
    pub shadow_draws: u32,
    /// Point/spot lights given a shadow in the atlas this frame (#468).
    pub shadowed_lights: u32,
    /// Point/spot lights that cast shadows and are shaded (the base camera's light
    /// budget keeps them, #873) but found no room in the atlas (#468): they shade
    /// unshadowed. The least important are dropped.
    pub shadow_lights_dropped: u32,
    /// Atlas tiles drawn this frame: one per spotlight, six per point light (#468).
    pub shadow_atlas_tiles: u32,
    /// Atlas texels those tiles cover, out of 2048² (#468).
    pub shadow_atlas_texels: u64,
    /// Atlas tiles whose static casters came from the static cache this frame (#694).
    pub shadow_atlas_cached: u32,
    /// Atlas tiles whose static casters were re-baked this frame (#694): the light
    /// moved, its tile moved or resized, or the cache was invalidated.
    pub shadow_atlas_rebaked: u32,
    /// UI batches drawn — backdrop batches (#426) included.
    pub ui_draws: u32,
    /// UI canvases meshed this frame (#2416), each once however many times a
    /// growing glyph atlas rebuilt it: `0` on a still HUD (`ui::kept`).
    pub ui_meshed: u32,
    /// Views whose UI layout was computed this frame (#2412): `0` while nothing
    /// the layout reads changed (`ui::kept::layout`).
    pub ui_laid_out: u32,
    /// Why it was (#2412), one per reason, in this order: a rebuild (a first frame,
    /// another World, an Undo, a dropped log), the screen or a Canvas, the texture
    /// sizes, the structure (a RectTransform, LayoutGroup, LayoutElement, parent,
    /// child order or active flag), the content (a sized Image or Text), a Transform,
    /// a marker (`ui::kept::layout::Why`). All `0` when the layout was kept.
    pub ui_relaid: [u32; 7],
    /// UI `Mask` coverage textures rendered (#428): one pass per visible Mask.
    pub ui_mask_passes: u32,
    /// Fullscreen passes the UI backdrop blur ran (#426): the downsampled
    /// composite, the down chain and each blurred level's way up. `0` when no
    /// backdrop is visible.
    pub ui_blur_passes: u32,
    /// Depth taps the SSAO occlusion pass traced (#436): its texels times its quality's
    /// samples, summed over the camera stack; `0` with AO off. Its depth prepass is
    /// counted in `draw_calls` and `triangles`.
    pub ssao_samples: u64,
    /// Samples the SSIL gather took (#1750): its texels times its quality's samples,
    /// on the base camera of each view running it; `0` with SSIL off. The prepass it
    /// draws on a camera without SSAO is counted in `draw_calls` and `triangles`.
    pub ssil_samples: u64,
    /// Froxels the volumetric fog lit (#1703): its grid, on the base camera of each
    /// view running it; `0` with it off.
    pub volumetric_froxels: u64,
    /// Particles drawn: sprite instances plus mesh particles, summed over the stack
    /// (#440). Their draw calls are in `draw_calls` — one per merged sprite batch, and
    /// one per instanced run of mesh particles.
    pub particles_drawn: u32,
    /// Render-texture cameras drawn this frame (#430). Their geometry is already in
    /// `draw_calls` / `triangles`; a camera skipped (unreferenced, or between its
    /// `update_every` frames) is not counted.
    pub render_texture_draws: u32,
    /// Opaque scene passes that wrote a velocity target (#1435): one per camera of
    /// each view running motion blur, `0` when none does.
    pub velocity_passes: u32,
    /// Opaque scene passes primed with the depth prepass's depth (#1499): one per
    /// camera that ran SSAO or SSIL, or that the GPU culls (#2336, its prepass
    /// then the late set alone), `0` with neither and no GPU cull. A camera shading with a forward
    /// override of its own (the shader preview) is never primed, nor is any on Metal.
    pub primed_passes: u32,
    /// Opaque scene pass batches of lit solids (those `BatchKey::in_prepass` admits)
    /// in a primed pass (#2334), summed over the stack: each shades only the pixels
    /// its own prepass depth kept.
    pub forward_lit_primed: u32,
    /// Opaque scene pass batches of lit solids in a pass that is not primed: their
    /// overdraw depends on what drew before them.
    pub forward_lit_unprimed: u32,
    /// Opaque scene pass batches of unlit solids (a light's own mesh, a gizmo
    /// solid), primed or not: they are in no prepass and write their own depth.
    pub forward_unlit: u32,
    /// Reasons this frame read history an earlier frame left (#2092), one per
    /// reason, not per view: each view (a render-texture camera's too) running an
    /// effect that keeps a history or motion vectors past a cut, each shown render
    /// texture (its picture kept from a draw, whatever its `update_every`), and a
    /// pending shadow tile resize. `0` means drawing the frame again with no sim
    /// step gives the same image.
    pub history_passes: u32,
    /// Opaque scene passes that wrote SSR's surface targets with the SSR pipelines
    /// (#1526): one per view whose base camera runs SSR. An overlay camera of the
    /// stack draws with the plain ones, so it never counts.
    pub ssr_surface_passes: u32,
    /// Views whose grading LUT was rebaked this frame (#1440): a grading knob or
    /// the tonemap changed, or the view is new. `0` on every frame the grade holds.
    pub grade_lut_bakes: u32,
    /// Sky reflection cubes baked this frame (#1553): a new panorama, or procedural
    /// sky settings no cube holds (#1620: the first procedural frame, or a settings
    /// change). `0` on every frame the sky holds.
    pub sky_reflection_bakes: u32,
    /// Opaque copies made this frame (#1523), Unity's *Opaque Texture*: one per
    /// camera whose visible translucents refract or absorb, or trace SSR on a camera
    /// running it (#1570); `0` on every other.
    pub opaque_copies: u32,
    /// GPU bytes of the image textures the renderer holds by path (material maps,
    /// UI images, particle textures, render textures) and the uncompressed copies
    /// a slot that cannot read a compressed one needs, every mip included: what
    /// texture compression and *Max Size* shrink (#1574). Not a per-frame count:
    /// the resident total at the frame's end.
    pub texture_bytes: u64,
    /// GPU bytes of every lightmap page array held (#1694): the bound colour and
    /// direction arrays and any other page list cached, every mip. The resident
    /// total at the frame's end, not a per-frame count.
    pub lightmap_bytes: u64,
    /// GPU bytes of the reflection probes' cube array (#2085): every allocated
    /// layer, spares included, every mip. The resident total at the frame's end.
    pub reflection_bytes: u64,
    /// GPU meshes resident (#2440): one per mesh identity some loaded scene, or
    /// a mesh particle of one, uses since the last load released the rest.
    pub mesh_count: u32,
    /// GPU bytes of their vertex and index buffers. The resident total.
    pub mesh_bytes: u64,
    /// GPU bytes the clustered meshes' leases hold of the cluster table and the
    /// index pool (`passes::hiz::clusters::pool`). The resident total.
    pub cluster_pool_bytes: u64,
    /// Retained render objects re-derived whole this frame (#1706): every mesh entity
    /// on a rebuild (a first frame, a load, an Undo, a renderer-side input), else the
    /// ones whose mesh, material, activity, layer, static flag or overrides changed.
    /// `0` on a frame that changed nothing.
    pub render_extracted: u32,
    /// Retained render objects only re-placed this frame: moved, nothing else (#1706).
    pub render_posed: u32,
    /// CPU microseconds the extract took (#1706), inside `renderer_ms`. Wall-clock.
    pub render_extract_us: u64,
    /// Bytes this frame wrote into the forward pass's per-draw group's arrays (#1706), every
    /// camera summed: per-draw uniforms, joints, instances, proxy volumes, and the
    /// object and material records that changed.
    pub render_upload_bytes: u64,
    /// The scene assets uploaded for this frame (`asset_uploads`, `asset_upload_bytes`).
    pub uploads: Uploads,
    /// The GPU calls this render made, from its start to its counters' close (`gpu::ops`).
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

/// A scene's baked PVS older than its statics at the last extract
/// (`Renderer::stale_occlusion`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaleOcclusion {
    /// Baked meshes that changed since the bake: always drawn.
    pub changed: usize,
    /// Of them, occluders: while any is, nothing is occlusion-culled.
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
