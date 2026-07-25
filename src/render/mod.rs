mod camera;
mod debug_meshes;
mod draw;
mod frustum;
mod preview;
pub mod readback;
mod setup;
mod view;

pub mod gpu;
pub(crate) mod ibl;
pub(crate) mod passes;
pub mod postfx;

// Moved submodules pulled back under short names so this module's body keeps
// naming them directly (grouped by subfolder — see the convention in CLAUDE.md).
use gpu::entity_pool;
use ibl::{cubemap, skybox};
use passes::{decals, particles, shadows};

use std::collections::HashMap;
use std::rc::Rc;

pub use camera::{build_camera_stack, game_camera_from_scene, sync_lens_from_scene, Camera};
pub use frustum::{transform_aabb, Frustum};
pub use ibl::cubemap_capture::{CubemapCapture, CubemapFace};
pub use ibl::probe_bake::{project_cubemap, DEFAULT_BAKE_RESOLUTION};
pub use ibl::probe_bounce::{BounceReport, CONVERGENCE_EPSILON, MAX_BOUNCES};
pub use ibl::reflection_bake::DEFAULT_REFLECTION_RESOLUTION;
pub use setup::headless::OFFSCREEN_FORMAT;
pub use view::RenderView;

// GPU uniform memory layouts live in `gpu/uniforms.rs` (split out to keep files under
// the size cap); re-imported here so the render module body still names them directly.
pub(crate) use gpu::uniforms::{
    AmbientLightUniform, BoneUniform, CameraUniform, DirectionalLightUniform, EntityUniform,
    LightingUniform, PointLightUniform, SpotlightUniform,
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
}

/// Stable identity for a mesh's GPU geometry, derived from its *source*
/// (`primitive_type` + `asset_ref`) rather than the entity referencing it (#127).
/// Many entities sharing the same primitive or imported asset therefore map to a
/// single `MeshId` and share one vertex/index buffer pair instead of allocating
/// N copies. The string form keeps a future runtime-mutated mesh expressible with
/// its own unique key (e.g. by entity id) without colliding with the shared ones.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct MeshId(pub String);

impl MeshId {
    /// Geometry key for a mesh component: `"<primitive_type>|<asset_ref>"`.
    pub fn from_mesh(mesh: &crate::scene::MeshComponent) -> Self {
        MeshId(format!(
            "{}|{}",
            mesh.primitive_type,
            mesh.asset_ref.as_deref().unwrap_or("")
        ))
    }
}

// Stores GPU handlers for textures
pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
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
    /// Alpha-blended translucent solids (#242); depth write off, drawn back-to-front
    /// in the transparent pass after opaque.
    transparent_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    outline_pipeline: wgpu::RenderPipeline,

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
    /// Persistent per-entity forward buffers + bind groups, written in place each frame
    /// instead of reallocated per camera per entity (#210). `Option` so a slot can be
    /// borrowed out mutably while other renderer fields are read.
    entity_pool: Option<entity_pool::EntityPool>,

    // Asset cache keyed by mesh-asset identity (#127): identical geometry shared
    // across entities resolves to one buffer pair, not one per entity.
    pub gpu_meshes: HashMap<MeshId, GpuMesh>,
    pub gpu_textures: HashMap<String, Rc<GpuTexture>>,
    pub default_texture: Rc<GpuTexture>,
    /// A group(2) material bind group whose three texture slots all point at
    /// `default_texture` — used by the outline and editor-grid passes, which never
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
    /// The active reflection probe's loaded, prefiltered cubemap (#245) at group-0 binding
    /// 4; `None` until a baked probe covers the camera, then the shader uses the skybox.
    /// Rebound only when `reflection_cube_path` (its cache key) changes, not every frame.
    reflection_cube: Option<Rc<cubemap::CubemapTexture>>,
    reflection_cube_path: String,
    /// Decoded cubemaps by path — the content cache behind `reflection_cube` (#355).
    /// Which cube is *bound* is per-scene state and legitimately flips when two scenes
    /// with different probes render in one frame; without this, each flip re-read the
    /// KTX2 from disk and re-uploaded every mip, i.e. a full disk load per frame. A
    /// cached `None` remembers an unreadable file so it is not retried on every flip
    /// either (the same shape as the audio `ClipCache`).
    cubemaps: HashMap<String, Option<Rc<cubemap::CubemapTexture>>>,
    /// A 1×1×6 black cube bound at binding 4 when no probe cube is active, so the bind
    /// group always satisfies the layout (the shader ignores it via `refl_has_cubemap`).
    default_cube: cubemap::CubemapTexture,
    pub shadow_layout: wgpu::BindGroupLayout,
    pub shadow_uniform_buffer: wgpu::Buffer,
    pub shadow_bind_group: wgpu::BindGroup,

    /// Active scalability tier; gates which post-FX passes run + buffer sizes. Shared
    /// across views — each view's post-FX chain (owned by its [`RenderView`]) sizes its
    /// bloom buffers from this tier's divisor.
    pub quality: postfx::QualityPreset,
    /// Present modes the surface advertises, captured at construction. Used to
    /// decide whether vsync can be turned off (`Immediate`) without a wgpu panic;
    /// the headless path has no real swapchain so it only ever lists `Fifo`.
    present_modes: Vec<wgpu::PresentMode>,

    /// Billboard particle pass (draws into the HDR target before post-FX).
    particle_renderer: particles::ParticleRenderer,

    /// Box-projector decal pass (draws into the HDR target after solids/skybox,
    /// reconstructing the underlying surface from the scene depth target).
    decal_renderer: decals::DecalRenderer,

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
}
