//! Resource construction for the post-process chain: bind-group layout, the
//! composite (assembled from the effects, `effects::assemble`), FXAA and copy
//! pipelines, the params and effect-slot buffers, sampler, and the render targets.

use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{
    depth_entry, sampler_entry, texture_entry, uniform_entry,
};
use crate::render::gpu::shaders::ShaderRegistry;

use super::effects::bloom::BloomTargets;
use super::effects::custom_effects::CustomChain;
use super::effects::exposure::ExposureMeter;
use super::effects::{assemble, composite_slots_len};
use super::grade::GradeLut;
use super::{PfxTarget, PostFx, PostParams, HDR_FORMAT};

/// The chain's built-in pipelines, named rather than positional — a tuple of
/// `RenderPipeline` is one chance per pass to wire the wrong one to the wrong pass.
struct Pipelines {
    composite: wgpu::RenderPipeline,
    fxaa: wgpu::RenderPipeline,
    copy: wgpu::RenderPipeline,
}

impl PostFx {
    /// Build the whole post-FX resource set sized for `width` x `height`, writing
    /// its final composite into `output_format` (the swapchain / screenshot format).
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        output_format: wgpu::TextureFormat,
        bloom_divisor: u32,
        registry: &mut ShaderRegistry,
    ) -> Self {
        let io_layout = Self::create_io_layout(device);
        let pipelines = Self::create_pipelines(device, &io_layout, output_format, registry);
        let (params_buffer, fx_buffer, sampler) = Self::create_params_and_sampler(device);

        let (full_size, scene_hdr, ldr) =
            Self::create_targets(device, width, height, output_format);
        let bloom = BloomTargets::new(device, full_size, bloom_divisor);
        let custom = CustomChain::new(device, &io_layout, output_format, full_size);
        let grade = GradeLut::new(device);
        let exposure = ExposureMeter::new(device);

        Self {
            composite_pipeline: pipelines.composite,
            fxaa_pipeline: pipelines.fxaa,
            copy_pipeline: pipelines.copy,
            grade,
            exposure,
            custom,
            io_layout,
            params_buffer,
            fx_buffer,
            sampler,
            scene_hdr,
            bloom,
            ldr,
            full_size,
            output_format,
            motion: Default::default(),
            targets: Default::default(),
        }
    }

    /// Build the built-in fullscreen passes (the assembled composite, FXAA, copy).
    fn create_pipelines(
        device: &wgpu::Device,
        io_layout: &wgpu::BindGroupLayout,
        output_format: wgpu::TextureFormat,
        registry: &mut ShaderRegistry,
    ) -> Pipelines {
        assemble::register_modules(registry);
        let composite =
            registry.load_source(device, &assemble::composite_source(), assemble::COMPOSITE);
        let fxaa = registry.load(device, "postfx/fxaa.wgsl", "PostFX FXAA Shader");
        let copy = registry.load(device, "postfx/copy.wgsl", "PostFX Copy Shader");

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PostFX Pipeline Layout"),
            bind_group_layouts: &[Some(io_layout)],
            immediate_size: 0,
        });
        // Each writes LDR: the composite into `ldr` (or straight to the output when
        // nothing follows it), FXAA and the copy always into the output.
        let pass = |shader: &wgpu::ShaderModule, fs: &str| {
            let entries = ("vs_fullscreen", fs);
            Self::fullscreen_pipeline(device, shader, &pipeline_layout, entries, output_format)
        };

        Pipelines {
            composite: pass(&composite, "fs_composite"),
            fxaa: pass(&fxaa, "fs_fxaa"),
            copy: pass(&copy, "fs_copy"),
        }
    }

    /// Create the params and effect-slot uniform buffers and the linear-clamp sampler.
    fn create_params_and_sampler(
        device: &wgpu::Device,
    ) -> (wgpu::Buffer, wgpu::Buffer, wgpu::Sampler) {
        let uniform = |label, size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let params_buffer = uniform("PostFX Params", std::mem::size_of::<PostParams>() as u64);
        let slots = composite_slots_len() * std::mem::size_of::<[f32; 4]>();
        let fx_buffer = uniform("PostFX Effect Slots", slots as u64);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("PostFX Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        (params_buffer, fx_buffer, sampler)
    }

    /// (Re)allocate the HDR + bloom + LDR targets for a new framebuffer size.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32, bloom_divisor: u32) {
        let (full_size, scene_hdr, ldr) =
            Self::create_targets(device, width, height, self.output_format);
        self.full_size = full_size;
        self.scene_hdr = scene_hdr;
        self.bloom = BloomTargets::new(device, full_size, bloom_divisor);
        self.ldr = ldr;
        self.custom.resize(device, full_size.0, full_size.1);
        // A resize is a cut (#1435): no smear from last frame's framing, and the
        // exposure snaps (#1444).
        self.motion.reset();
        self.exposure.cut();
        // Every effect's own targets refit at the new size the next frame it runs.
        self.targets.clear();
    }

    fn create_targets(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        output_format: wgpu::TextureFormat,
    ) -> ((u32, u32), PfxTarget, PfxTarget) {
        let w = width.max(1);
        let h = height.max(1);
        let scene_hdr = Self::target(device, w, h, "PostFX Scene HDR", HDR_FORMAT);
        // Full resolution and in the *output* format: FXAA samples the finished frame
        // exactly as it would have been presented.
        let ldr = Self::target(device, w, h, "PostFX LDR", output_format);
        ((w, h), scene_hdr, ldr)
    }

    pub(crate) fn target(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        label: &str,
        format: wgpu::TextureFormat,
    ) -> PfxTarget {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            // COPY_*: the authored-effect chain keeps its result as next frame's
            // history (#397) by copying one LDR target into another.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        PfxTarget { texture, view }
    }

    /// A fullscreen-triangle pass: `(vs, fs)` entry points of `shader`, writing one
    /// `format` target with no blending.
    pub(crate) fn fullscreen_pipeline(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
        (vs_entry, fs_entry): (&str, &str),
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let pass = Fullscreen {
            label: "PostFX Pipeline",
            shader,
            layout,
            vs: vs_entry,
            fs: fs_entry,
        };
        pass.colour(
            device,
            &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        )
    }

    fn create_io_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PostFX IO Layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                texture_entry(1, wgpu::TextureViewDimension::D2),
                sampler_entry(2),
                texture_entry(3, wgpu::TextureViewDimension::D2),
                // Depth is read via textureLoad on a depth view of the
                // Depth32Float target (point sampling — no sampler needed).
                depth_entry(4, wgpu::TextureViewDimension::D2),
                texture_entry(5, wgpu::TextureViewDimension::D2),
                // The skybox's own sampler: the panorama wraps at its seam (#995).
                sampler_entry(6),
                // The grading LUT (#1440), sampled with the linear-clamp sampler.
                texture_entry(7, wgpu::TextureViewDimension::D3),
                // The adapted exposure (#1444), read in Automatic.
                uniform_entry(8, wgpu::ShaderStages::FRAGMENT),
                // The composite's effect slots (#1467).
                uniform_entry(9, wgpu::ShaderStages::FRAGMENT),
            ],
        })
    }
}
