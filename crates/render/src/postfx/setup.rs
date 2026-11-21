use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::layout_entries::{depth_entry, sampler_entry, texture_entry, uniform_entry};
use crate::gpu::shaders::ShaderRegistry;

use super::effects::custom_effects::CustomChain;
use super::effects::exposure::ExposureMeter;
use super::effects::textures::CompositeTextures;
use super::effects::{assemble, composite_slots_len};
use super::grade::GradeLut;
use super::{PfxTarget, PostFx, PostParams, HDR_FORMAT};

struct Pipelines {
    composite: wgpu::RenderPipeline,
    fxaa: wgpu::RenderPipeline,
    copy: wgpu::RenderPipeline,
}

impl PostFx {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        output_format: wgpu::TextureFormat,
        registry: &mut ShaderRegistry,
    ) -> Self {
        let io_layout = Self::create_io_layout(device);
        let effect_textures = CompositeTextures::new(device);
        let layouts = (&io_layout, &effect_textures.layout);
        let pipelines = Self::create_pipelines(device, layouts, output_format, registry);
        let (params_buffer, fx_buffer, sampler) = Self::create_params_and_sampler(device);

        let (full_size, scene_hdr, ldr) =
            Self::create_targets(device, width, height, output_format);
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
            effect_textures,
            params_buffer,
            fx_buffer,
            sampler,
            scene_hdr,
            bloom_dirt: None,
            ldr,
            full_size,
            chain_size: full_size,
            output_format,
            motion: Default::default(),
            targets: Default::default(),
        }
    }

    fn create_pipelines(
        device: &wgpu::Device,
        (io_layout, textures): (&wgpu::BindGroupLayout, &wgpu::BindGroupLayout),
        output_format: wgpu::TextureFormat,
        registry: &mut ShaderRegistry,
    ) -> Pipelines {
        assemble::register_modules(registry);
        let composite =
            registry.load_source(device, &assemble::composite_source(), assemble::COMPOSITE);
        let fxaa = registry.load(device, "postfx/fxaa.wgsl", "PostFX FXAA Shader");
        let copy = registry.load(device, "postfx/copy.wgsl", "PostFX Copy Shader");

        let pipeline_layout = |label, groups: &[Option<&wgpu::BindGroupLayout>]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: groups,
                immediate_size: 0,
            })
        };
        let io = pipeline_layout("PostFX Pipeline Layout", &[Some(io_layout)]);
        let groups = [Some(io_layout), Some(textures)];
        let with_textures = pipeline_layout("PostFX Composite Pipeline Layout", &groups);
        let pass = |shader: &wgpu::ShaderModule, fs: &str, layout: &wgpu::PipelineLayout| {
            let entries = ("vs_fullscreen", fs);
            Self::fullscreen_pipeline(device, shader, layout, entries, output_format)
        };

        Pipelines {
            composite: pass(&composite, "fs_composite", &with_textures),
            fxaa: pass(&fxaa, "fs_fxaa", &io),
            copy: pass(&copy, "fs_copy", &io),
        }
    }

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

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let (w, h) = (width.max(1), height.max(1));
        self.full_size = (w, h);
        self.scene_hdr = Self::target(device, w, h, "PostFX Scene HDR", HDR_FORMAT);
        self.chain_size = (0, 0);
        self.motion.reset();
        self.exposure.cut();
        self.targets.clear();
    }

    pub(crate) fn fit_chain(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        let size = (size.0.max(1), size.1.max(1));
        if size == self.chain_size {
            return;
        }
        self.chain_size = size;
        self.ldr = Self::target(device, size.0, size.1, "PostFX LDR", self.output_format);
        self.custom.resize(device, size.0, size.1);
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        PfxTarget { texture, view }
    }

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
                depth_entry(4, wgpu::TextureViewDimension::D2),
                texture_entry(5, wgpu::TextureViewDimension::D2),
                sampler_entry(6),
                texture_entry(7, wgpu::TextureViewDimension::D3),
                uniform_entry(8, wgpu::ShaderStages::FRAGMENT),
                uniform_entry(9, wgpu::ShaderStages::FRAGMENT),
            ],
        })
    }
}
