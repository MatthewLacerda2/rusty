//! GPU-resource construction for [`Renderer`], split out of `setup.rs`.
//!
//! Each builder here returns a small *cohesive* group of related GPU resources —
//! the global camera/lighting bindings, the shadow system, the forward passes, the
//! billboard passes — rather than one flat struct that mirrors every `Renderer`
//! field verbatim. `Renderer::from_parts` (in `setup.rs`) destructures these groups
//! straight into the assembled renderer, so the seams follow real boundaries (what
//! depends on what) instead of a pass-through layer (#211). Behavior is unchanged.

use crate::core::quality::QualityPreset;
use crate::render::clusters::ClusterBuffers;
use crate::render::decals::DecalBuffers;
use crate::render::gpu::bind_layouts;
use crate::render::gpu::global_group::GlobalGroup;
use crate::render::gpu::lightmaps::Lightmaps;
use crate::render::gpu::pipelines;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::HDR_FORMAT;
use crate::render::setup::textures::{create_textures, Textures};
use crate::render::{ibl::skybox, passes::shadows, passes::ssao};
use crate::render::{CameraUniform, GpuTexture, LightingUniform};

/// Camera + lighting uniform buffers and the group(0) bind group.
pub(crate) struct GlobalBindings {
    pub camera_buffer: wgpu::Buffer,
    pub lighting_buffer: wgpu::Buffer,
    pub global_bind_group: wgpu::BindGroup,
    /// The light-cluster buffers group 0 binds (#434).
    pub clusters: ClusterBuffers,
    /// The decal records and atlas group 0 binds (#638).
    pub decals: DecalBuffers,
    /// The baked lightmap pages group 0 binds (#438).
    pub lightmaps: Lightmaps,
}

/// Shadow renderer, its cascade uniform buffer, and the main-pass bind group that
/// samples the active cascade array.
pub(crate) struct ShadowSystem {
    pub layout: wgpu::BindGroupLayout,
    pub renderer: shadows::ShadowRenderer,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

/// The forward-lit, line, outline and skybox passes — everything that draws into
/// the HDR offscreen target.
pub(crate) struct ForwardPasses {
    pub render_pipeline: wgpu::RenderPipeline,
    pub transparent_pipeline: wgpu::RenderPipeline,
    pub line_pipeline: wgpu::RenderPipeline,
    pub outline_pipeline: wgpu::RenderPipeline,
    pub prepass_pipeline: wgpu::RenderPipeline,
    pub skybox_renderer: skybox::SkyboxRenderer,
}

/// Billboard particle and ribbon (#441) passes; both reuse the renderer's
/// `texture_layout`, and the particles sample the scene depth through
/// `scene_depth_layout`.
pub(crate) struct BillboardPasses {
    pub particle_renderer: crate::render::passes::particles::ParticleRenderer,
    pub ribbon_renderer: crate::render::passes::ribbons::RibbonRenderer,
    pub scene_depth_layout: wgpu::BindGroupLayout,
}

/// Every non-trivial GPU resource a [`Renderer`] needs, grouped by role. Built in
/// dependency order (layouts → textures → bindings → passes) and consumed once by
/// [`Renderer::from_parts`], which moves each group into the flat renderer.
pub(crate) struct GpuResources {
    pub camera_lighting_layout: wgpu::BindGroupLayout,
    pub entity_bones_layout: wgpu::BindGroupLayout,
    pub textures: Textures,
    pub global: GlobalBindings,
    pub shadows: ShadowSystem,
    pub forward: ForwardPasses,
    pub billboards: BillboardPasses,
    /// The in-game UI pass (#418).
    pub ui: crate::render::ui::UiRenderer,
    /// The SSAO passes (#436).
    pub ssao: ssao::SsaoRenderer,
    pub quality: QualityPreset,
}

impl GpuResources {
    /// Build every *shared* GPU resource from an already-created device/queue. The
    /// per-view targets (depth + post-FX chain) are no longer built here — each
    /// [`crate::render::RenderView`] owns its own (#355).
    pub(crate) fn build(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let mut registry = ShaderRegistry::new(crate::shadergen::engine_shader_dir());
        let camera_lighting_layout = bind_layouts::create_camera_lighting_layout(device);
        let entity_bones_layout = bind_layouts::create_entity_bones_layout(device);
        let textures = create_textures(device, queue);
        let global =
            create_global_bindings(device, &camera_lighting_layout, &textures.default_texture);
        let ssao = ssao::SsaoRenderer::new(device, queue, &mut registry);
        // One module draws the solids and their shadows (#648).
        let shader = registry.load(device, "shader.wgsl", "Forward Lit Shader");
        let atlas_shader = registry.load(device, "shadow_atlas.wgsl", "Shadow Atlas Shader");
        let shadow_shaders = [&shader, &atlas_shader];
        let shadows = create_shadow_system(device, shadow_shaders, &ssao.no_ao, &textures);
        let forward = create_forward_passes(
            device,
            &shader,
            [
                &camera_lighting_layout,
                &entity_bones_layout,
                &shadows.layout,
            ],
            &textures,
            &mut registry,
        );
        let billboards = create_billboard_passes(
            device,
            queue,
            &textures.texture_layout,
            &camera_lighting_layout,
            &mut registry,
        );
        let ui = crate::render::ui::UiRenderer::new(device, queue, &mut registry);
        Self {
            camera_lighting_layout,
            entity_bones_layout,
            textures,
            global,
            shadows,
            forward,
            billboards,
            ui,
            ssao,
            quality: QualityPreset::default(),
        }
    }
}

/// Billboard particle and ribbon (#441) passes; both reuse the renderer's
/// `texture_layout`, and the particles also read the scene depth and the camera +
/// lighting group 0 (#440).
fn create_billboard_passes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture_layout: &wgpu::BindGroupLayout,
    camera_lighting_layout: &wgpu::BindGroupLayout,
    registry: &mut ShaderRegistry,
) -> BillboardPasses {
    use crate::render::passes::particles::{ParticleRenderer, SharedLayouts};
    let scene_depth_layout = bind_layouts::create_scene_depth_layout(device);
    let layouts = SharedLayouts {
        texture: texture_layout,
        depth: &scene_depth_layout,
        camera_lighting: camera_lighting_layout,
    };
    BillboardPasses {
        particle_renderer: ParticleRenderer::new(device, layouts, registry),
        ribbon_renderer: crate::render::passes::ribbons::RibbonRenderer::new(
            device,
            queue,
            texture_layout,
            registry,
        ),
        scene_depth_layout,
    }
}

/// Camera + lighting uniform buffers, the light clusters and the group-0 bind group
/// (also binds the default texture/sampler).
fn create_global_bindings(
    device: &wgpu::Device,
    camera_lighting_layout: &wgpu::BindGroupLayout,
    default_texture: &GpuTexture,
) -> GlobalBindings {
    let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Camera Uniform Buffer"),
        size: std::mem::size_of::<CameraUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let lighting_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Lighting Uniform Buffer"),
        size: std::mem::size_of::<LightingUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // A throwaway fallback cube satisfies binding 4 until the per-frame rebuild swaps in
    // the active reflection probe's cubemap (or the renderer's own fallback). The shader
    // ignores it (the `refl_has_cubemap` flag stays 0) and samples the 2D skybox instead.
    let cube = crate::render::ibl::cubemap::fallback_cube(device);
    let clusters = ClusterBuffers::new(device);
    let decals = DecalBuffers::new(device);
    let lightmaps = Lightmaps::new(device);
    let global_bind_group = GlobalGroup {
        camera: &camera_buffer,
        lighting: &lighting_buffer,
        skybox: (&default_texture.view, &default_texture.sampler),
        cube: (&cube.view, &cube.sampler),
        clusters: &clusters,
        decals: &decals,
        lightmaps: lightmaps.binding(),
    }
    .create(device, camera_lighting_layout);

    GlobalBindings {
        camera_buffer,
        lighting_buffer,
        global_bind_group,
        clusters,
        decals,
        lightmaps,
    }
}

/// Shadow renderer (over the forward shader and the atlas's blits, `shaders`), its
/// cascade uniform buffer, and the main-pass bind group that samples the active
/// cascade array — with `no_ao`, the white stand-in for SSAO.
fn create_shadow_system(
    device: &wgpu::Device,
    shaders: [&wgpu::ShaderModule; 2],
    no_ao: &wgpu::TextureView,
    textures: &Textures,
) -> ShadowSystem {
    let layout = bind_layouts::create_shadow_layout(device);
    let renderer = shadows::ShadowRenderer::new(device, shaders, &textures.material_layout);

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Shadow Uniform Buffer"),
        size: std::mem::size_of::<shadows::CascadeUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group =
        bind_layouts::create_shadow_bind_group(device, &layout, &uniform_buffer, &renderer, no_ao);

    ShadowSystem {
        layout,
        renderer,
        uniform_buffer,
        bind_group,
    }
}

/// Build the forward-lit, line, outline and skybox passes that all draw into the HDR
/// offscreen target, the solids through the forward `shader`.
/// `layouts` are groups 0, 1 and 3: camera + lighting, entity + bones, shadows.
fn create_forward_passes(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    [camera_lighting_layout, entity_bones_layout, shadow_layout]: [&wgpu::BindGroupLayout; 3],
    textures: &Textures,
    registry: &mut ShaderRegistry,
) -> ForwardPasses {
    // Skybox binds a single-texture `GpuTexture.bind_group` at its group(1), so it
    // keeps the single `texture_layout`; the forward pipelines use `material_layout`.
    let skybox_renderer = skybox::SkyboxRenderer::new(
        device,
        &textures.texture_layout,
        camera_lighting_layout,
        HDR_FORMAT,
        registry,
    );

    let pl = pipelines::create_pipelines(
        device,
        shader,
        HDR_FORMAT,
        camera_lighting_layout,
        entity_bones_layout,
        &textures.material_layout,
        shadow_layout,
    );

    ForwardPasses {
        render_pipeline: pl.forward,
        transparent_pipeline: pl.transparent,
        line_pipeline: pl.line,
        outline_pipeline: pl.outline,
        prepass_pipeline: pl.prepass,
        skybox_renderer,
    }
}
