//! Resource construction for the post-process chain: bind-group layout, the three
//! fullscreen pipelines, the params buffer, sampler, and the render targets.

use crate::render::shaders::ShaderRegistry;

use super::{PfxTarget, PostFx, PostParams, HDR_FORMAT};

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
        let (bright_pipeline, blur_pipeline, composite_pipeline) =
            Self::create_pipelines(device, &io_layout, output_format, registry);
        let (params_buffer, sampler) = Self::create_params_and_sampler(device);

        let (full_size, bloom_size, scene_hdr, bloom_a, bloom_b) =
            Self::create_targets(device, width, height, bloom_divisor);

        Self {
            bright_pipeline,
            blur_pipeline,
            composite_pipeline,
            io_layout,
            params_buffer,
            sampler,
            scene_hdr,
            bloom_a,
            bloom_b,
            bloom_size,
            full_size,
            prev_view_proj: glam::Mat4::IDENTITY,
        }
    }

    /// Build the three fullscreen passes (bright extract, blur, final composite).
    fn create_pipelines(
        device: &wgpu::Device,
        io_layout: &wgpu::BindGroupLayout,
        output_format: wgpu::TextureFormat,
        registry: &mut ShaderRegistry,
    ) -> (
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
    ) {
        let shader = registry.load(device, "postfx.wgsl", "PostFX Shader");

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PostFX Pipeline Layout"),
            bind_group_layouts: &[io_layout],
            push_constant_ranges: &[],
        });

        let bright_pipeline =
            Self::pipeline(device, &shader, &pipeline_layout, "fs_bright", HDR_FORMAT);
        let blur_pipeline =
            Self::pipeline(device, &shader, &pipeline_layout, "fs_blur", HDR_FORMAT);
        let composite_pipeline = Self::pipeline(
            device,
            &shader,
            &pipeline_layout,
            "fs_composite",
            output_format,
        );

        (bright_pipeline, blur_pipeline, composite_pipeline)
    }

    /// Create the params uniform buffer and the linear-clamp sampler.
    fn create_params_and_sampler(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Sampler) {
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PostFX Params"),
            size: std::mem::size_of::<PostParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("PostFX Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        (params_buffer, sampler)
    }

    /// (Re)allocate the HDR + bloom targets for a new framebuffer size.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32, bloom_divisor: u32) {
        let (full_size, bloom_size, scene_hdr, bloom_a, bloom_b) =
            Self::create_targets(device, width, height, bloom_divisor);
        self.full_size = full_size;
        self.bloom_size = bloom_size;
        self.scene_hdr = scene_hdr;
        self.bloom_a = bloom_a;
        self.bloom_b = bloom_b;
    }

    fn create_targets(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        bloom_divisor: u32,
    ) -> ((u32, u32), (u32, u32), PfxTarget, PfxTarget, PfxTarget) {
        let w = width.max(1);
        let h = height.max(1);
        let bw = (w / bloom_divisor).max(1);
        let bh = (h / bloom_divisor).max(1);
        let scene_hdr = Self::target(device, w, h, "PostFX Scene HDR");
        let bloom_a = Self::target(device, bw, bh, "PostFX Bloom A");
        let bloom_b = Self::target(device, bw, bh, "PostFX Bloom B");
        ((w, h), (bw, bh), scene_hdr, bloom_a, bloom_b)
    }

    fn target(device: &wgpu::Device, width: u32, height: u32, label: &str) -> PfxTarget {
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
            format: HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        PfxTarget { texture, view }
    }

    fn pipeline(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
        fs_entry: &str,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PostFX Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: fs_entry,
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        })
    }

    fn create_io_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        let tex = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PostFX IO Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                tex(3),
                // Depth is read via textureLoad on a depth view of the
                // Depth32Float target (point sampling — no sampler needed).
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                tex(5),
            ],
        })
    }
}
