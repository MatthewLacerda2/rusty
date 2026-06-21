pub mod mesh;
pub mod postfx;
pub mod shaders;
pub mod shadows;
pub mod skybox;

mod bind_layouts;
mod camera;
mod debug_meshes;
pub mod decals;
mod decals_draw;
mod draw;
mod draw_lighting;
mod draw_overlays;
mod draw_pass;
mod draw_path;
mod draw_resources;
mod particles;
mod particles_draw;
mod pipelines;
mod postfx_params;
mod setup;
mod setup_build;
mod setup_headless;
mod setup_resize;
mod setup_textures;
mod textures;

use std::collections::HashMap;
use std::rc::Rc;

pub use camera::{build_camera_stack, sync_lens_from_scene, Camera};
pub use setup_headless::OFFSCREEN_FORMAT;

// Represent memory layouts for GPU Uniforms
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [f32; 16],
    camera_pos: [f32; 3],
    _pad: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct AmbientLightUniform {
    color: [f32; 3],
    intensity: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct DirectionalLightUniform {
    direction: [f32; 3],
    _pad1: f32,
    color: [f32; 3],
    intensity: f32,
    _pad2: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct PointLightUniform {
    position: [f32; 3],
    _pad1: f32,
    color: [f32; 3],
    intensity: f32,
    range: f32,
    _pad2: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SpotlightUniform {
    position: [f32; 3],
    _pad1: f32,
    direction: [f32; 3],
    _pad2: f32,
    color: [f32; 3],
    intensity: f32,
    range: f32,
    inner_cone: f32,
    outer_cone: f32,
    _pad3: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct LightingUniform {
    ambient: AmbientLightUniform,
    dir_light: DirectionalLightUniform,
    point_lights: [PointLightUniform; 4],
    spot_light: SpotlightUniform,
    num_point_lights: u32,
    ssr_active: f32,
    ssr_quality: f32,
    ssr_temporal_upsampling: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct EntityUniform {
    model_matrix: [f32; 16],
    color_tint: [f32; 4],
    use_texture: u32,
    is_lit: u32,
    metallic: f32,
    roughness: f32,
    // Flags for the metallic/roughness maps (#202). When set, the shader multiplies
    // the scalar by the sampled map channel. The two `u32` pads fill the run of u32s
    // up to the next 16-byte boundary so the `vec4` that follows is aligned;
    // `EntityUniforms` in shader.wgsl mirrors this field order byte-for-byte.
    use_metallic_map: u32,
    use_roughness_map: u32,
    _pad0: u32,
    _pad1: u32,
    // Flat emissive factor (#222): rgb glow added on top of the lit colour in
    // `fs_main`. The renderer draws to an HDR target with a bloom bright-pass, so
    // values >1.0 glow automatically. Stored as a `vec4` (4th lane unused) to dodge
    // WGSL's `vec3` 16-byte-alignment gotcha and keep the struct a 16-byte multiple
    // (112 -> 128 bytes).
    emissive: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct BoneUniform {
    bones: [[f32; 16]; 64],
}

// Stores GPU Buffer handlers for meshes
pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
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
    pub size: winit::dpi::PhysicalSize<u32>,

    // Pipelines
    render_pipeline: wgpu::RenderPipeline,
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

    // Depth Stencil
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,

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
    pub shadow_layout: wgpu::BindGroupLayout,
    pub shadow_uniform_buffer: wgpu::Buffer,
    pub shadow_bind_group: wgpu::BindGroup,

    /// Post-process chain (color correction, bloom, motion blur, SSR).
    pub post_fx: postfx::PostFx,
    /// Active scalability tier; gates which post-FX passes run + buffer sizes.
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
}

#[cfg(test)]
mod mesh_id_tests {
    use super::MeshId;
    use crate::scene::{DirtyFlag, MeshComponent};

    fn mesh(primitive: &str, asset: Option<&str>) -> MeshComponent {
        MeshComponent {
            primitive_type: primitive.to_string(),
            asset_ref: asset.map(String::from),
            vertices: Vec::new(),
            indices: Vec::new(),
            bind_palette: Vec::new(),
            skin: None,
            clips: Vec::new(),
            pose_palette: Vec::new(),
            is_dirty: DirtyFlag::new(false),
        }
    }

    #[test]
    fn identical_geometry_shares_one_id() {
        // The key is the source, not the entity: two box meshes dedup to one
        // buffer; two references to the same asset sub-object likewise.
        assert_eq!(
            MeshId::from_mesh(&mesh("Box", None)),
            MeshId::from_mesh(&mesh("Box", None))
        );
        assert_eq!(
            MeshId::from_mesh(&mesh("Asset", Some("models/crates.glb::Barrel"))),
            MeshId::from_mesh(&mesh("Asset", Some("models/crates.glb::Barrel"))),
        );
    }

    #[test]
    fn distinct_geometry_gets_distinct_ids() {
        assert_ne!(
            MeshId::from_mesh(&mesh("Box", None)),
            MeshId::from_mesh(&mesh("Sphere", None))
        );
        assert_ne!(
            MeshId::from_mesh(&mesh("Asset", Some("models/crates.glb::Barrel"))),
            MeshId::from_mesh(&mesh("Asset", Some("models/crates.glb::Crate"))),
        );
    }
}
