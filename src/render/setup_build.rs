//! GPU-resource construction for [`Renderer`], split out of `setup.rs` so the
//! flat `from_parts` assembly stays small. Every field of [`RendererParts`]
//! moves verbatim into the corresponding `Renderer` field; this module only
//! groups the builders, it changes no behavior.

use std::rc::Rc;

use super::pipelines;
use super::postfx::{PostFx, QualityPreset, HDR_FORMAT};
use super::shaders::ShaderRegistry;
use super::{shadows, skybox};
use super::{CameraUniform, GpuTexture, LightingUniform, Renderer};

/// All non-trivial GPU resources for a [`Renderer`], built before assembly.
pub(super) struct RendererParts {
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub camera_lighting_layout: wgpu::BindGroupLayout,
    pub entity_bones_layout: wgpu::BindGroupLayout,
    pub texture_layout: wgpu::BindGroupLayout,
    pub default_texture: Rc<GpuTexture>,
    pub camera_buffer: wgpu::Buffer,
    pub lighting_buffer: wgpu::Buffer,
    pub global_bind_group: wgpu::BindGroup,
    pub render_pipeline: wgpu::RenderPipeline,
    pub line_pipeline: wgpu::RenderPipeline,
    pub outline_pipeline: wgpu::RenderPipeline,
    pub skybox_renderer: skybox::SkyboxRenderer,
    pub shadow_renderer: shadows::ShadowRenderer,
    pub shadow_layout: wgpu::BindGroupLayout,
    pub shadow_uniform_buffer: wgpu::Buffer,
    pub shadow_bind_group: wgpu::BindGroup,
    pub post_fx: PostFx,
    pub quality: QualityPreset,
    pub particle_renderer: super::particles::ParticleRenderer,
    pub decal_renderer: super::decals::DecalRenderer,
}

impl RendererParts {
    /// Build every GPU resource from an already-created device/queue/config.
    pub(super) fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
    ) -> Self {
        let mut registry = ShaderRegistry::new("assets/shaders");

        let (depth_texture, depth_view) = Renderer::create_depth_resources(device, config);

        let camera_lighting_layout = pipelines::create_camera_lighting_layout(device);
        let entity_bones_layout = pipelines::create_entity_bones_layout(device);
        let texture_layout = pipelines::create_texture_layout(device);

        // Default grid checker texture (bound statically to the global layout).
        let default_texture = Rc::new(Renderer::create_default_checkerboard_texture(
            device,
            queue,
            &texture_layout,
        ));

        let (camera_buffer, lighting_buffer, global_bind_group) =
            create_global_bindings(device, &camera_lighting_layout, &default_texture);

        let shadow_layout = pipelines::create_shadow_layout(device);
        let (shadow_renderer, shadow_uniform_buffer, shadow_bind_group) =
            create_shadow_system(device, &shadow_layout, &mut registry);

        let (render_pipeline, line_pipeline, outline_pipeline, skybox_renderer) =
            create_forward_passes(
                device,
                &camera_lighting_layout,
                &entity_bones_layout,
                &texture_layout,
                &shadow_layout,
                &mut registry,
            );

        let (post_fx, quality) = create_post_chain(device, config, &mut registry);

        Self {
            depth_texture,
            depth_view,
            camera_lighting_layout,
            entity_bones_layout,
            // Billboard particle + box-projector decal passes reuse the layout.
            particle_renderer: super::particles::ParticleRenderer::new(device, &texture_layout, &mut registry),
            decal_renderer: super::decals::DecalRenderer::new(device, &texture_layout, &mut registry),
            texture_layout,
            default_texture,
            camera_buffer,
            lighting_buffer,
            global_bind_group,
            render_pipeline,
            line_pipeline,
            outline_pipeline,
            skybox_renderer,
            shadow_renderer,
            shadow_layout,
            shadow_uniform_buffer,
            shadow_bind_group,
            post_fx,
            quality,
        }
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
) -> (wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup) {
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
        ],
    });

    (camera_buffer, lighting_buffer, global_bind_group)
}

/// Shadow renderer, its light-space uniform buffer, and the main-pass bind group
/// that samples the active shadow map.
fn create_shadow_system(
    device: &wgpu::Device,
    shadow_layout: &wgpu::BindGroupLayout,
    registry: &mut ShaderRegistry,
) -> (shadows::ShadowRenderer, wgpu::Buffer, wgpu::BindGroup) {
    let shadow_renderer = shadows::ShadowRenderer::new(device, registry);

    let shadow_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Shadow Uniform Buffer"),
        size: 64, // Mat4 size
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Main Shadow Bind Group"),
        layout: shadow_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: shadow_uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&shadow_renderer.active_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&shadow_renderer.sampler),
            },
        ],
    });

    (shadow_renderer, shadow_uniform_buffer, shadow_bind_group)
}

/// Compile the forward shader and build the forward-lit, line, outline and skybox
/// passes that all draw into the HDR offscreen target.
fn create_forward_passes(
    device: &wgpu::Device,
    camera_lighting_layout: &wgpu::BindGroupLayout,
    entity_bones_layout: &wgpu::BindGroupLayout,
    texture_layout: &wgpu::BindGroupLayout,
    shadow_layout: &wgpu::BindGroupLayout,
    registry: &mut ShaderRegistry,
) -> (
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
    skybox::SkyboxRenderer,
) {
    let shader = registry.load(device, "shader.wgsl", "Forward Lit Shader");

    let skybox_renderer = skybox::SkyboxRenderer::new(
        device,
        texture_layout,
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
        texture_layout,
        shadow_layout,
    );

    (
        render_pipeline,
        line_pipeline,
        outline_pipeline,
        skybox_renderer,
    )
}
