#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "platform layer, exempt from the sim's clock ban (#757): GPU timing is wall-clock"
)]

mod camera;
pub(crate) mod clusters;
mod debug_meshes;
pub(crate) mod decals;
mod draw;
mod frustum;
pub(crate) mod lod;
pub(crate) mod motion;
mod preview;
pub mod readback;
pub(crate) mod render_texture;
mod setup;
mod stats;
#[cfg(test)]
pub(crate) mod test_gpu;
pub(crate) mod timing;
mod view;

pub mod gpu;
pub(crate) mod ibl;
pub(crate) mod passes;
pub mod postfx;
pub mod ui;

// Moved submodules pulled back under short names so this module's body keeps
// naming them directly (grouped by subfolder — see the convention in CLAUDE.md).
use ibl::{cubemap, reflection_probes, sky_cube, skybox};
use passes::{particles, shadows};

use crate::components::MeshRevision;
use std::collections::HashMap;
use std::rc::Rc;

pub use camera::{build_camera_stack, texture_cameras, ScreenStack, TextureCamera};
pub use frustum::{Frustum, SkinBounds};
pub use gpu::mesh::MeshId;
pub use ibl::cubemap_capture::{CubemapCapture, CubemapFace};
pub use ibl::probe::bake::{project_cubemap, DEFAULT_BAKE_RESOLUTION};
pub use ibl::probe::bounce::{BounceReport, CONVERGENCE_EPSILON, MAX_BOUNCES};
pub use ibl::reflection_bake::DEFAULT_REFLECTION_RESOLUTION;
pub use setup::headless::{gpu_required, OFFSCREEN_FORMAT, REQUIRE_GPU_ENV};
pub use setup::SurfaceWindow;
pub use stats::RenderCounters;
pub use timing::{ClockProbe, GpuPass, GpuTimes};
pub use view::{PixelSize, RenderView};

// GPU uniform memory layouts live in `gpu/uniforms.rs` (split out to keep files under
// the size cap); re-imported here so the render module body still names them directly.
pub(crate) use gpu::uniforms::{
    AmbientLightUniform, CameraUniform, DirectionalLightUniform, EntityUniform, InstanceData,
    LightingUniform,
};

// Stores GPU Buffer handlers for meshes
pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
    /// Local-space AABB `(min, max)` over this geometry's vertices, computed once at
    /// upload and stored alongside the vertex data (#330). Frustum culling transforms
    /// it by the entity's world matrix (8 corners, O(1)) each frame instead of ever
    /// re-walking the vertices.
    pub local_aabb: (glam::Vec3, glam::Vec3),
    /// Per-joint bind-pose boxes, computed at the same upload: a skinned entity's
    /// animated bound is built from them and its palette each frame (#833).
    pub skin_bounds: SkinBounds,
    /// The geometry revision these buffers hold (#1540): a mesh carrying a newer
    /// one is re-uploaded.
    pub revision: MeshRevision,
}

impl GpuMesh {
    /// The draws of `mesh` under `material` (#1285): each material slot with the
    /// index run it draws ([`crate::components::mesh::material_runs`]), clipped to this
    /// geometry's indices so a mesh edited ahead of its upload never reads past the
    /// buffer ([`crate::components::mesh::clip_runs`]).
    pub(crate) fn runs(
        &self,
        mesh: &crate::scene::MeshComponent,
        material: Option<&crate::components::MaterialComponent>,
    ) -> Vec<(usize, std::ops::Range<u32>)> {
        let runs = crate::components::mesh::clip_runs(mesh.index_ranges(), self.num_indices);
        crate::components::mesh::material_runs(runs, material.map(|m| m.materials.len()))
    }
}

// Stores GPU handlers for textures
pub struct GpuTexture {
    pub texture: wgpu::Texture,
    /// The colour view: an sRGB-stored image decodes to linear on sample. What
    /// albedo, emissive, UI, decals and particles bind.
    pub view: wgpu::TextureView,
    /// The same texels read as stored, with no sRGB decode (#647): what a material's
    /// data maps (metallic, roughness, normal, the shader texture slots) bind.
    pub data_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    /// The filter `sampler` reads with: a UI image samples with it too, but always
    /// clamped (`render::ui`, #1397).
    pub filter: crate::asset::sidecar::FilterMode,
    pub bind_group: wgpu::BindGroup,
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// `None` in the headless (offscreen screenshot) path — there is no window
    /// surface to present to, only an offscreen colour texture.
    pub surface: Option<wgpu::Surface<'static>>,
    pub config: wgpu::SurfaceConfiguration,

    // Pipelines
    render_pipeline: wgpu::RenderPipeline,
    /// Forward pipelines also writing velocity (#1435) and SSR surface (#1526) targets.
    velocity_pipeline: gpu::pipelines::VelocityPipelines,
    /// The opaque ones over a view's primed depth (#1499).
    primed_pipelines: gpu::pipelines::PrimedPipelines,
    /// The post effects' shared pipelines (#1467): bloom, depth of field (#1445), the
    /// exposure meter (#1444) and motion blur (#1436).
    pub(crate) post_passes: postfx::effects::Passes,
    /// Alpha-blended translucent solids (#242); depth write off, drawn back-to-front
    /// in the transparent pass after opaque. Built on first use, like the three
    /// editor-only pipelines below (#985).
    transparent_pipelines: gpu::pipelines::TransparentPipelines,
    line_pipeline: gpu::pipelines::LazyPipeline,
    /// The editor's selection outline (#1175).
    outline: passes::outline::OutlinePasses,

    // Bind Group Layouts
    pub camera_lighting_layout: wgpu::BindGroupLayout,
    pub entity_bones_layout: wgpu::BindGroupLayout,
    pub texture_layout: wgpu::BindGroupLayout,
    /// Expanded group(2) layout (albedo + sampler + metallic + roughness maps) the
    /// forward pass binds a per-entity material bind group against (#202).
    pub material_layout: wgpu::BindGroupLayout,

    // Buffers and Bind Groups (Global)
    camera_buffer: wgpu::Buffer,
    lighting_buffer: wgpu::Buffer,
    pub global_bind_group: wgpu::BindGroup,
    /// Rebuild the group-0 bind group only when the skybox it binds changed, not every
    /// camera every frame — its camera/lighting buffers are persistent (#210).
    global_bind_group_dirty: bool,
    /// The light-cluster buffers group 0 binds (#434), refilled per camera.
    clusters: clusters::ClusterBuffers,
    /// The baked lightmap pages group 0 binds (#438), rebound when the scene's change.
    pub(crate) lightmaps: gpu::lightmaps::Lightmaps,
    /// The packed per-frame draw data every solid draw binds as group 1 (#470): per-draw
    /// uniforms, bone palettes and instances, rewritten per camera.
    draw_buffers: gpu::draw_buffers::DrawBuffers,
    /// One group-2 material bind group per distinct set of resolved maps (#470).
    materials: gpu::material_cache::MaterialCache,
    /// The pipelines materials' authored surface shaders draw with, by name (#396).
    pub(crate) surface_shaders: gpu::pipelines::surface::SurfaceShaders,
    /// Group repeated mesh + material pairs into instanced draws (#470). Always on in
    /// the engine; tests turn it off to compare against one-draw-per-entity.
    pub(crate) instancing: bool,
    /// Reuse the SSAO prepass's depth in the forward pass (#1499). Always on in the
    /// engine; tests turn it off to compare against the unprimed pass.
    pub(crate) depth_priming: bool,
    /// Whether the adapter's backend primes at all (`pipelines::priming`): not Metal.
    pub(crate) priming_recommended: bool,

    // Asset cache keyed by mesh-asset identity (#127): identical geometry shared
    // across entities resolves to one buffer pair, not one per entity.
    pub gpu_meshes: HashMap<MeshId, GpuMesh>,
    pub gpu_textures: HashMap<crate::asset::MapRef, Rc<GpuTexture>>,
    /// Which cached uploads went stale: re-written files and failed loads (#689).
    /// What each cached upload's mips were built for (#1416).
    pub(crate) texture_kinds: crate::render::gpu::textures::kind_watch::KindWatch,
    pub(crate) texture_freshness: crate::core::fs::freshness::Freshness,
    pub default_texture: Rc<GpuTexture>,
    /// What an extra shader texture slot samples with no texture of its own (#400).
    pub white_texture: Rc<GpuTexture>,
    /// Draws every uploaded image's mip chain (#1322).
    pub(crate) mip_generator: gpu::textures::MipGenerator,
    /// A group(2) material bind group whose three texture slots all point at
    /// `default_texture` — used by the editor-grid and gizmo passes, which never
    /// sample the maps (they render unlit / use the default checker) but still must
    /// supply a bind group matching the expanded `material_layout` (#202).
    pub default_material_bind_group: wgpu::BindGroup,

    grid_vertex_buffer: Option<wgpu::Buffer>,
    grid_count: u32,

    axis_x_buffer: Option<wgpu::Buffer>,
    axis_y_buffer: Option<wgpu::Buffer>,
    axis_z_buffer: Option<wgpu::Buffer>,
    axis_count: u32,

    pub skybox_renderer: skybox::SkyboxRenderer,
    pub shadow_renderer: shadows::ShadowRenderer,
    pub skybox_texture: Option<Rc<GpuTexture>>,
    pub skybox_path: String,
    /// The reflection probes the frame shades with (#1206): the nearest baked probes'
    /// cubes in one cube array at group-0 binding 4, and the slot each draw reads.
    reflection_probes: reflection_probes::ProbeArray,
    /// The sky's prefiltered reflection cubes at group-0 binding 2 (#1553).
    sky_reflection: sky_cube::SkyReflection,
    /// Decoded cubemaps by path — the content cache the probe array copies from (#355).
    /// Which probes are resident is per-scene state and legitimately flips when two
    /// scenes with different probes render in one frame; without this, each flip
    /// re-read the KTX2 from disk and re-uploaded every mip. A cached `None` remembers
    /// an unreadable file so it is not retried on every flip either; an engine write to
    /// the path (a reflection bake) drops the entry, cached `None` included
    /// (`refresh_textures`, #1207).
    cubemaps: HashMap<String, Option<Rc<cubemap::CubemapTexture>>>,
    pub shadow_layout: wgpu::BindGroupLayout,
    pub shadow_uniform_buffer: wgpu::Buffer,
    /// The forward pass's group 3 with the white no-AO fallback: every pass that
    /// draws without SSAO (the transparent pass, a frame with SSAO off) binds this.
    pub shadow_bind_group: wgpu::BindGroup,
    /// The opaque copy's pass, stand-in and sampler (#1523): every group 3 binds the
    /// stand-in but a transparent pass whose camera copied
    /// (`passes::transparent::opaque_copy`).
    pub(crate) opaque_copy: passes::transparent::OpaqueCopyPass,
    /// The SSAO depth prepass (#436): the forward vertex stage, depth only.
    pub(crate) prepass_pipeline: wgpu::RenderPipeline,
    /// The SSAO occlusion + blur passes and their no-AO fallback (#436).
    pub(crate) ssao: passes::ssao::SsaoRenderer,
    /// The grading-LUT bake every view's table is written through (#1440).
    pub(crate) grade_bake: postfx::grade::GradeBake,

    /// Active scalability tier; gates which post-FX passes run, and each view's
    /// post-FX chain (owned by its [`RenderView`]) sizes its bloom buffers from it.
    pub quality: crate::core::settings::quality::QualityPreset,
    /// The player's graphics options (#1438): which effects the player allows,
    /// ANDed with each camera's and the tier's choice (`postfx::params::effect_runs`).
    pub graphics_options: crate::core::settings::graphics_options::GraphicsOptions,
    /// The screen's render scale (#1498); see [`RenderView::set_render_scale`].
    pub render_scale: crate::core::settings::render_scale::RenderScale,
    /// Present modes the surface advertises, captured at construction. Used to
    /// decide whether vsync can be turned off (`Immediate`) without a wgpu panic;
    /// the headless path has no real swapchain so it only ever lists `Fifo`.
    present_modes: Vec<wgpu::PresentMode>,

    /// Billboard particle pass (draws into the HDR target before post-FX).
    particle_renderer: particles::ParticleRenderer,

    /// Trail + line ribbon pass (#441), drawn just before the particles.
    ribbon_renderer: passes::ribbons::RibbonRenderer,

    /// The in-game UI pass (#418), drawn over the finished frame after post-FX.
    ui_renderer: ui::UiRenderer,

    /// The frame's decals and their atlas (#638), bound in group 0 and folded into
    /// the forward pass's material inputs.
    decals: decals::DecalBuffers,

    /// The scene-depth layout the soft particles sample the depth target through.
    scene_depth_layout: wgpu::BindGroupLayout,

    /// When set, the forward pass gathers only `is_static` entities (#243). The
    /// static-cubemap capture toggles this on for its 6 faces and restores it after,
    /// so probe/reflection bakes see the distant environment (static walls + skybox)
    /// without the dynamic actors that would otherwise bake into the captured lighting.
    static_capture: bool,

    /// When set, static surfaces sample the probe field during a static-cubemap
    /// capture instead of falling back to flat ambient (#285). The multi-bounce probe
    /// bake toggles this on for bounce ≥2 so each capture re-lights the static scene
    /// from the previous bounce's probe SH, adding one indirect bounce per pass.
    /// Off everywhere else, so direct-only capture (bounce 1, reflection bakes) and
    /// runtime shading keep their "static ⇒ no probe SH" rule.
    capture_probe_bounce: bool,

    /// What the last `render` call submitted (#433) — read by the dev layer's stats.
    pub frame_counters: RenderCounters,
    /// GPU time per render pass (#835): disabled without `TIMESTAMP_QUERY`.
    pub(crate) gpu_timer: timing::GpuTimer,

    /// Every registered render texture's current allocation id, by path, and the
    /// last id handed out (#430) — the material cache's key for `"rt:"` maps.
    render_texture_ids: (HashMap<String, u64>, u64),

    /// This renderer's slot in the headless budget (#366) — `Some` for headless
    /// renderers, `None` for the windowed one, which is the application itself and
    /// never queues behind a test budget.
    ///
    /// **Declared last on purpose.** Struct fields drop in declaration order, so the
    /// permit is released only after the device, queue and every GPU resource above it
    /// have been dropped. Freeing the slot any earlier would let the next renderer
    /// allocate against memory this one has not actually returned yet — which is the
    /// entire failure this guard exists to prevent.
    _gpu_permit: Option<setup::budget::Permit>,
}
