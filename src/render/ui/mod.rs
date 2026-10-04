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
//! - `clip` — what clips a graphic: RectMask bounds and feather, the nearest Mask
//!   (pure).
//! - `mesh` — the layout → one vertex list per canvas, split into batches on
//!   texture / clip changes, with CanvasGroup alpha and the clips (pure).
//! - `effects` — the batch group every batch binds: soft clips, Mask coverage
//!   textures (#428) and the backdrop blur (#426).
//! - `vertex` — the one vertex format every graphic shares, and fill/gradient encoding.
//! - `shape` — a `Shape`'s SDF quads: shadow, glow, body (#425).
//! - `blend` — the blend modes as GPU blend state (#425).
//! - `pipeline` — the screen and world pipeline shapes, built in one place.
//! - `custom` — custom ui shaders (#427): a graphic's shade as its batch carries it
//!   (pure), the variant cache and the per-graphic uniforms.
//! - `text` — SDF glyph generation, the per-font atlases and text quads.
//! - `cache` — the per-view vertex buffers (re-upload only a canvas whose
//!   geometry changed).
//! - `draw` — preparing a view's UI and the screen pass itself.
//! - `world` — world canvases (`WorldSpace`, `ScreenSpaceCamera`) drawn in the
//!   scene, inside each camera's pass (#429).
//!
//! **Colour space.** UI blends in display (sRGB-encoded) space with premultiplied
//! alpha: the pass draws through a non-sRGB *view* of the frame's sRGB colour
//! target (see `docs/ui.md` for why this beats an in-shader encode).

pub(crate) mod blend;
pub(crate) mod cache;
pub(crate) mod clip;
pub(crate) mod custom;
pub(crate) mod draw;
pub(crate) mod effects;
pub(crate) mod geometry;
pub(crate) mod mesh;
pub(crate) mod pipeline;
pub(crate) mod shape;
pub(crate) mod text;
pub(crate) mod vertex;
pub(crate) mod world;

use std::collections::HashMap;
use std::rc::Rc;

use crate::components::UiBlend;
use crate::render::gpu::bind_layouts;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::GpuTexture;

pub use cache::UiViewCache;

/// Every shared GPU resource of the UI pass. One per `Renderer`; pipelines are
/// built lazily per target format (the window's and the headless one differ).
pub struct UiRenderer {
    shader: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    /// One pipeline per (target format, blend mode).
    pipelines: HashMap<(wgpu::TextureFormat, UiBlend), wgpu::RenderPipeline>,
    /// The backdrop batches' pipeline (#426), per target format.
    backdrops: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
    /// Linear, clamped, base-level-only: UI sprites are drawn near 1:1, so mips
    /// would only blur them.
    sampler: wgpu::Sampler,
    /// A 1×1 white texture, sampled by solid-colour graphics.
    white: wgpu::BindGroup,
    /// Its view: the mask and backdrop of a batch that has neither.
    white_view: wgpu::TextureView,
    /// Every batch's group 1: clip, mask and backdrop (`effects`).
    batch_layout: wgpu::BindGroupLayout,
    /// Draws a Mask's graphic into its coverage texture (#428).
    mask_pipeline: wgpu::RenderPipeline,
    /// The backdrop's blur chain (#426).
    blur: effects::blur::BlurPipelines,
    /// Bind groups per texture path, kept with the texture they bind so a reloaded
    /// texture rebinds.
    textures: HashMap<String, (Rc<GpuTexture>, wgpu::BindGroup)>,
    /// Every font's glyph atlas (CPU side, shared by all views).
    atlases: text::atlas::FontAtlases,
    /// Each atlas's GPU copy, by font path.
    fonts: HashMap<Option<String>, text::gpu::AtlasGpu>,
    /// The world-canvas pipeline (#429).
    world: world::WorldUiPipeline,
    /// Custom ui shaders' pipelines (#427).
    shaders: custom::UiShaders,
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
        let batch_layout = effects::batch_layout(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("UI Pipeline Layout"),
            bind_group_layouts: &[Some(&layout), Some(&batch_layout)],
            immediate_size: 0,
        });
        let mask_pipeline = effects::mask_gpu::mask_pipeline(device, &shader, &pipeline_layout);
        let blur_shader = registry.load(device, "ui_blur.wgsl", "UI Blur Shader");
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("UI Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            lod_max_clamp: 0.0,
            ..Default::default()
        });
        let white_view = white_texture(device, queue);
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
        }
    }

    /// Build the pipelines for `format` if absent, one per blend mode, plus the
    /// backdrop's (#426, always "over"): premultiplied colour, no depth, no culling
    /// (a clockwise radial fill winds the other way).
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

    /// The screen pipeline drawing into `format` with `mode` through `entry`.
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

    /// The bind group a batch drawing `source` samples (white when it is not loaded).
    fn source_group(&self, source: &mesh::UiSource) -> &wgpu::BindGroup {
        let group = match source {
            mesh::UiSource::Solid => None,
            mesh::UiSource::Texture(p) => self.textures.get(p).map(|(_, g)| g),
            mesh::UiSource::Font(f) => self.fonts.get(f).map(|a| &a.group),
        };
        group.unwrap_or(&self.white)
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

impl crate::render::Renderer {
    /// The view of a swapchain frame the UI pass draws through, and its format: the
    /// frame's non-sRGB twin when the surface was configured with one (display-space
    /// blending), else the frame itself. Hand it to [`RenderView::set_ui_output`].
    ///
    /// [`RenderView::set_ui_output`]: crate::render::RenderView::set_ui_output
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

/// A texture + the UI sampler as a bind group against `layout`.
pub(crate) fn bind(
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
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: None,
        },
        size,
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
