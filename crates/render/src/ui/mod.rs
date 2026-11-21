pub(crate) mod blend;
pub(crate) mod cache;
pub(crate) mod clip;
pub(crate) mod custom;
pub(crate) mod draw;
pub(crate) mod effects;
pub(crate) mod geometry;
pub(crate) mod kept;
pub(crate) mod mesh;
pub(crate) mod pipeline;
pub(crate) mod shape;
pub(crate) mod text;
pub(crate) mod vertex;
pub(crate) mod world;

use std::collections::HashMap;
use std::rc::Rc;

use crate::gpu::bind_layouts;
use crate::gpu::ops::{CountedDevice as _, CountedQueue as _, Writes};
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::GpuTexture;
use rusty_asset::sidecar::FilterMode;
use rusty_components::UiBlend;

pub use cache::UiViewCache;

pub struct UiRenderer {
    shader: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    pipelines: HashMap<(wgpu::TextureFormat, UiBlend), wgpu::RenderPipeline>,
    backdrops: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
    sampler: wgpu::Sampler,
    image_samplers: HashMap<FilterMode, wgpu::Sampler>,
    white: wgpu::BindGroup,
    white_view: wgpu::TextureView,
    batch_layout: wgpu::BindGroupLayout,
    mask_pipeline: wgpu::RenderPipeline,
    blur: effects::blur::BlurPipelines,
    textures: HashMap<String, (Rc<GpuTexture>, wgpu::BindGroup)>,
    atlases: text::atlas::FontAtlases,
    fonts: HashMap<Option<String>, text::gpu::AtlasGpu>,
    world: world::WorldUiPipeline,
    shaders: custom::UiShaders,
    texture_sizes: Rc<rusty_ui::TextureSizes>,
}

impl UiRenderer {
    pub(crate) fn new(
        device: &wgpu::Device,
        writes: Writes,
        registry: &mut ShaderRegistry,
    ) -> Self {
        let shader = registry.load(device, "ui.wgsl", "UI Shader");
        let layout = bind_layouts::create_texture_layout(device);
        let batch_layout = effects::batch_layout(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("UI Pipeline Layout"),
            bind_group_layouts: &[Some(&layout), Some(&batch_layout)],
            immediate_size: 0,
        });
        let mask_pipeline = effects::mask::gpu::mask_pipeline(device, &shader, &pipeline_layout);
        let blur_shader = registry.load(device, "ui_blur.wgsl", "UI Blur Shader");
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("UI Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            lod_max_clamp: 0.0,
            ..Default::default()
        });
        let white_view = white_texture(device, writes);
        let white = bind(device, &layout, &white_view, &sampler);
        let world = world::WorldUiPipeline::new(device, &shader, &[&layout, &batch_layout]);
        let shaders = custom::UiShaders::new(device, [&layout, &batch_layout, &world.layout]);
        Self {
            shader,
            layout,
            pipeline_layout,
            pipelines: HashMap::new(),
            backdrops: HashMap::new(),
            sampler,
            image_samplers: HashMap::new(),
            white,
            white_view,
            batch_layout,
            mask_pipeline,
            blur: effects::blur::BlurPipelines::new(device, blur_shader),
            textures: HashMap::new(),
            atlases: Default::default(),
            fonts: HashMap::new(),
            world,
            shaders,
            texture_sizes: Rc::default(),
        }
    }

    pub(crate) fn lazy_pipelines(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.world.lazy()
    }

    fn ensure_pipeline(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        for mode in UiBlend::ALL {
            if !self.pipelines.contains_key(&(format, mode)) {
                let pipeline = self.build_pipeline(device, format, (mode, "fs_main"));
                self.pipelines.insert((format, mode), pipeline);
            }
        }
        if !self.backdrops.contains_key(&format) {
            let pipeline = self.build_pipeline(device, format, (UiBlend::Normal, "fs_backdrop"));
            self.backdrops.insert(format, pipeline);
        }
    }

    fn build_pipeline(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        (mode, entry): (UiBlend, &str),
    ) -> wgpu::RenderPipeline {
        let shader = ("UI Pipeline", &self.shader, &self.pipeline_layout);
        pipeline::build_entry(
            device,
            shader,
            pipeline::UiPass::Screen(format),
            (mode, entry),
        )
    }

    fn source_group(&self, source: &mesh::UiSource) -> &wgpu::BindGroup {
        let group = match source {
            mesh::UiSource::Solid => None,
            mesh::UiSource::Texture(p) => self.textures.get(p).map(|(_, g)| g),
            mesh::UiSource::Font(f) => self.fonts.get(f).map(|a| &a.group),
        };
        group.unwrap_or(&self.white)
    }
}

impl crate::Renderer {
    pub fn set_ui_textures(&mut self, textures: Rc<rusty_ui::TextureSizes>) {
        self.ui_renderer.texture_sizes = textures;
    }

    pub fn surface_ui_view(
        &self,
        frame: &wgpu::Texture,
    ) -> (wgpu::TextureView, wgpu::TextureFormat) {
        let format = self
            .config
            .view_formats
            .first()
            .copied()
            .unwrap_or(self.config.format);
        let view = frame.create_view(&wgpu::TextureViewDescriptor {
            format: Some(format),
            ..Default::default()
        });
        (view, format)
    }
}

pub(crate) fn bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.counted_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("UI Texture Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn white_texture(device: &wgpu::Device, (queue, staging): Writes) -> wgpu::TextureView {
    let size = wgpu::Extent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("UI White"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.counted_write_texture(
        staging,
        texture.as_image_copy(),
        &[255; 4],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: None,
        },
        size,
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
