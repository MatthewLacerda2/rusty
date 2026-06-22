//! GPU-resource construction for [`Renderer`], split out of `setup.rs`.
//!
//! Each builder here returns a small *cohesive* group of related GPU resources —
//! the global camera/lighting bindings, the shadow system, the forward passes, the
//! billboard passes — rather than one flat struct that mirrors every `Renderer`
//! field verbatim. `Renderer::from_parts` (in `setup.rs`) destructures these groups
//! straight into the assembled renderer, so the seams follow real boundaries (what
//! depends on what) instead of a pass-through layer (#211). Behavior is unchanged.

use super::bind_layouts;
use super::pipelines;
use super::postfx::{PostFx, QualityPreset, HDR_FORMAT};
use super::setup_textures::{create_textures, Textures};
use super::shaders::ShaderRegistry;
use super::{shadows, skybox};
use super::{CameraUniform, GpuTexture, LightingUniform, Renderer};

/// Camera + lighting uniform buffers and the group(0) bind group.
pub(super) struct GlobalBindings {
    pub camera_buffer: wgpu::Buffer,
    pub lighting_buffer: wgpu::Buffer,
    pub global_bind_group: wgpu::BindGroup,
}

/// Shadow renderer, its light-space uniform buffer, and the main-pass bind group
/// that samples the active shadow map.
pub(super) struct ShadowSystem {
    pub layout: wgpu::BindGroupLayout,
    pub renderer: shadows::ShadowRenderer,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

/// The forward-lit, line, outline and skybox passes — everything that draws into
/// the HDR offscreen target.
pub(super) struct ForwardPasses {
    pub render_pipeline: wgpu::RenderPipeline,
    pub line_pipeline: wgpu::RenderPipeline,
    pub outline_pipeline: wgpu::RenderPipeline,
    pub skybox_renderer: skybox::SkyboxRenderer,
}

/// Billboard particle + box-projector decal passes; both reuse the renderer's
/// `texture_layout` (group 1 / group 3 respectively).
pub(super) struct BillboardPasses {
    pub particle_renderer: super::particles::ParticleRenderer,
    pub decal_renderer: super::decals::DecalRenderer,
}

/// Every non-trivial GPU resource a [`Renderer`] needs, grouped by role. Built in
/// dependency order (layouts → textures → bindings → passes) and consumed once by
/// [`Renderer::from_parts`], which moves each group into the flat renderer.
pub(super) struct GpuResources {
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub camera_lighting_layout: wgpu::BindGroupLayout,
    pub entity_bones_layout: wgpu::BindGroupLayout,
    pub textures: Textures,
    pub global: GlobalBindings,
    pub shadows: ShadowSystem,
    pub forward: ForwardPasses,
    pub billboards: BillboardPasses,
    pub post_fx: PostFx,
    pub quality: QualityPreset,
}

impl GpuResources {
    /// Build every GPU resource from an already-created device/queue/config.
    pub(super) fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
    ) -> Self {
        let mut registry = ShaderRegistry::new("assets/shaders");
        let (depth_texture, depth_view) = Renderer::create_depth_resources(device, config);
        let camera_lighting_layout = bind_layouts::create_camera_lighting_layout(device);
        let entity_bones_layout = bind_layouts::create_entity_bones_layout(device);
        let textures = create_textures(device, queue);
        let global =
            create_global_bindings(device, &camera_lighting_layout, &textures.default_texture);
        let shadows = create_shadow_system(device, &mut registry);
        let forward = create_forward_passes(
            device,
            &camera_lighting_layout,
            &entity_bones_layout,
            &textures,
            &shadows.layout,
            &mut registry,
        );
        let (post_fx, quality) = create_post_chain(device, config, &mut registry);
        let billboards = create_billboard_passes(device, &textures.texture_layout, &mut registry);
        Self {
            depth_texture,
            depth_view,
            camera_lighting_layout,
            entity_bones_layout,
            textures,
            global,
            shadows,
            forward,
            billboards,
            post_fx,
            quality,
        }
    }
}

/// Billboard particle + box-projector decal passes; both reuse the renderer's
/// `texture_layout` (group 1 / group 3 respectively).
fn create_billboard_passes(
    device: &wgpu::Device,
    texture_layout: &wgpu::BindGroupLayout,
    registry: &mut ShaderRegistry,
) -> BillboardPasses {
    BillboardPasses {
        particle_renderer: super::particles::ParticleRenderer::new(
            device,
            texture_layout,
            registry,
        ),
        decal_renderer: super::decals::DecalRenderer::new(device, texture_layout, registry),
    }
}

/// Post-process chain sized to the framebuffer, plus the default quality tier.
fn create_post_chain(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    registry: &mut ShaderRegistry,
) -> (PostFx, QualityPreset) {
    let quality = QualityPreset::default();
    let post_fx = PostFx::new(
        device,
        config.width,
        config.height,
        config.format,
        quality.bloom_divisor(),
        registry,
    );
    (post_fx, quality)
}

/// Camera + lighting uniform buffers and the group-0 bind group (also binds the
/// default texture/sampler).
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
    let cube = super::cubemap::fallback_cube(device);
    let global_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Global Bind Group"),
        layout: camera_lighting_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: lighting_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&default_texture.view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&default_texture.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&cube.view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(&cube.sampler),
            },
        ],
    });

    GlobalBindings {
        camera_buffer,
        lighting_buffer,
        global_bind_group,
    }
}

/// Shadow renderer, its light-space uniform buffer, and the main-pass bind group
/// that samples the active shadow map.
fn create_shadow_system(device: &wgpu::Device, registry: &mut ShaderRegistry) -> ShadowSystem {
    let layout = bind_layouts::create_shadow_layout(device);
    let renderer = shadows::ShadowRenderer::new(device, registry);

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Shadow Uniform Buffer"),
        size: 64, // Mat4 size
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Main Shadow Bind Group"),
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&renderer.active_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&renderer.sampler),
            },
        ],
    });

    ShadowSystem {
        layout,
        renderer,
        uniform_buffer,
        bind_group,
    }
}

/// Compile the forward shader and build the forward-lit, line, outline and skybox
/// passes that all draw into the HDR offscreen target.
fn create_forward_passes(
    device: &wgpu::Device,
    camera_lighting_layout: &wgpu::BindGroupLayout,
    entity_bones_layout: &wgpu::BindGroupLayout,
    textures: &Textures,
    shadow_layout: &wgpu::BindGroupLayout,
    registry: &mut ShaderRegistry,
) -> ForwardPasses {
    let shader = registry.load(device, "shader.wgsl", "Forward Lit Shader");

    // Skybox binds a single-texture `GpuTexture.bind_group` at its group(1), so it
    // keeps the single `texture_layout`; the forward pipelines use `material_layout`.
    let skybox_renderer = skybox::SkyboxRenderer::new(
        device,
        &textures.texture_layout,
        camera_lighting_layout,
        HDR_FORMAT,
        registry,
    );

    let (render_pipeline, line_pipeline, outline_pipeline) = pipelines::create_pipelines(
        device,
        &shader,
        HDR_FORMAT,
        camera_lighting_layout,
        entity_bones_layout,
        &textures.material_layout,
        shadow_layout,
    );

    ForwardPasses {
        render_pipeline,
        line_pipeline,
        outline_pipeline,
        skybox_renderer,
    }
}
