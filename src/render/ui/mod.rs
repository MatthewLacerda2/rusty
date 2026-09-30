//! src/render/ui/ — the in-game UI render pass (#418).
//!
//! The GPU half of the UI. The sim half (`crate::ui`) lays canvases out on the CPU;
//! this module draws the result as **one batched 2D pass after the whole post-FX
//! chain**, onto the frame the editor's Game view, headless screenshots and the
//! player all present — so a HUD is never tonemapped, bloomed, motion-blurred or
//! FXAA-softened. The arrow is render → sim: this reads the layout, never the
//! reverse.
//!
//! - `geometry` — one `Image`'s triangles per image type (pure).
//! - `mesh` — the layout → one vertex list per canvas, split into batches on
//!   texture / clip changes, with CanvasGroup alpha and RectMask clips (pure).
//! - `draw` — the per-view cache (re-upload only a canvas whose geometry
//!   changed) and the pass itself.
//!
//! **Colour space.** UI blends in display (sRGB-encoded) space with premultiplied
//! alpha: the pass draws through a non-sRGB *view* of the frame's sRGB colour
//! target (see `docs/ui.md` for why this beats an in-shader encode).

pub(crate) mod draw;
pub(crate) mod geometry;
pub(crate) mod mesh;

use std::collections::HashMap;
use std::rc::Rc;

use crate::render::gpu::bind_layouts;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::GpuTexture;

pub use draw::UiViewCache;

/// Every shared GPU resource of the UI pass. One per `Renderer`; pipelines are
/// built lazily per target format (the window's and the headless one differ).
pub struct UiRenderer {
    shader: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
    /// Linear, clamped, base-level-only: UI sprites are drawn near 1:1, so mips
    /// would only blur them.
    sampler: wgpu::Sampler,
    /// A 1×1 white texture, sampled by solid-colour graphics.
    white: wgpu::BindGroup,
    /// Bind groups per texture path, kept with the texture they bind so a reloaded
    /// texture rebinds.
    textures: HashMap<String, (Rc<GpuTexture>, wgpu::BindGroup)>,
}

impl UiRenderer {
    /// Load the UI shader and build the shared layout, sampler and white texture.
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        registry: &mut ShaderRegistry,
    ) -> Self {
        let shader = registry.load(device, "ui.wgsl", "UI Shader");
        let layout = bind_layouts::create_texture_layout(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("UI Pipeline Layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("UI Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            lod_max_clamp: 0.0,
            ..Default::default()
        });
        let white = white_texture(device, queue);
        let white = bind(device, &layout, &white, &sampler);
        Self {
            shader,
            layout,
            pipeline_layout,
            pipelines: HashMap::new(),
            sampler,
            white,
            textures: HashMap::new(),
        }
    }

    /// Build the pipeline for `format` if absent: premultiplied-alpha blending, no
    /// depth, no culling (a clockwise radial fill winds the other way).
    fn ensure_pipeline(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        if self.pipelines.contains_key(&format) {
            return;
        }
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI Pipeline"),
            layout: Some(&self.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: "vs_main",
                buffers: &[draw::vertex_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });
        self.pipelines.insert(format, pipeline);
    }

    /// The bind group sampling `texture` (the white one for `None`), creating and
    /// caching it on first use or when the texture behind the path changed.
    fn bind_group(&mut self, device: &wgpu::Device, texture: Option<(&str, Rc<GpuTexture>)>) {
        let Some((path, tex)) = texture else { return };
        let fresh = self
            .textures
            .get(path)
            .is_some_and(|(cached, _)| Rc::ptr_eq(cached, &tex));
        if !fresh {
            let group = bind(device, &self.layout, &tex.view, &self.sampler);
            self.textures.insert(path.to_string(), (tex, group));
        }
    }
}

/// A texture + the UI sampler as a bind group against `layout`.
fn bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
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

/// A 1×1 opaque white sRGB texture's view.
fn white_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
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
    queue.write_texture(
        texture.as_image_copy(),
        &[255; 4],
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: None,
        },
        size,
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
