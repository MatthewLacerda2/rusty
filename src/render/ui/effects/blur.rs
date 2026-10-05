//! src/render/ui/effects/blur.rs — the backdrop's dual-filter blur (#426).
//!
//! **Where it reads the scene.** The overlay pass draws onto the finished frame —
//! often a swapchain image, which can be neither copied nor sampled. So the blur
//! does not copy the frame: after the camera stack's post-FX (the last stage of
//! the per-camera table, #636), it re-runs the post-FX *composite* over the view's
//! HDR scene colour straight into the chain's first level, at 1/`divisor` of the
//! view's size (`QualityPreset::backdrop_divisor`). That is the graded,
//! tonemapped, bloomed 3D frame — everything but FXAA and the authored effects,
//! which a blur would erase anyway — and not UI drawn before it. A fullscreen pass
//! at reduced resolution doubles as the first downsample.
//!
//! **The chain.** Down from level 0 to the deepest level any backdrop needs (shared
//! by every level), then, per distinct level `n`, up from level `n` back to level
//! 0's size into that level's own result (`ui_blur.wgsl`). One blur per distinct
//! level per frame, across every canvas; none at all when no backdrop is visible.
//! Every target is in the frame's own (sRGB) format, so blur taps filter in linear
//! light; the UI pass samples a result through its non-sRGB view, getting the
//! display-encoded values it blends in.

use std::collections::HashMap;

use super::backdrop::blur_level;
use crate::render::postfx::PostFxContext;
use crate::render::timing::GpuTimer;
use crate::render::{RenderView, Renderer};
use crate::ui::CanvasSpace;

/// One blur target: the view passes render and sample through, and the non-sRGB
/// view the UI pass reads (a view keeps its texture alive).
struct Level {
    view: wgpu::TextureView,
    raw: wgpu::TextureView,
}

fn level(device: &wgpu::Device, format: wgpu::TextureFormat, (w, h): (u32, u32)) -> Level {
    let raw_format = format.remove_srgb_suffix();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("UI Backdrop Blur"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[raw_format],
    });
    let view = texture.create_view(&Default::default());
    let raw = texture.create_view(&wgpu::TextureViewDescriptor {
        format: Some(raw_format),
        ..Default::default()
    });
    Level { view, raw }
}

/// The size of chain level `k` when level 0 is `base`.
fn size_at((w, h): (u32, u32), k: u32) -> (u32, u32) {
    ((w >> k).max(1), (h >> k).max(1))
}

/// A view's blur chain: the down levels (index = level), the up scratch levels
/// (index = level − 1) and one result per blurred level.
pub(crate) struct BlurTargets {
    format: wgpu::TextureFormat,
    base: (u32, u32),
    downs: Vec<Level>,
    ups: Vec<Level>,
    results: HashMap<u32, Level>,
}

impl BlurTargets {
    fn new(format: wgpu::TextureFormat, base: (u32, u32)) -> Self {
        Self {
            format,
            base,
            downs: Vec::new(),
            ups: Vec::new(),
            results: HashMap::new(),
        }
    }

    /// The blurred frame at `level`, as the UI pass reads it.
    pub(crate) fn result(&self, level: u32) -> Option<&wgpu::TextureView> {
        match level {
            0 => self.downs.first().map(|l| &l.raw),
            n => self.results.get(&n).map(|l| &l.raw),
        }
    }

    /// Grow the chain to `deepest` and keep a result for exactly `levels`.
    fn ensure(&mut self, device: &wgpu::Device, deepest: u32, levels: &[u32]) {
        let (format, base) = (self.format, self.base);
        while self.downs.len() <= deepest as usize {
            let k = self.downs.len() as u32;
            self.downs.push(level(device, format, size_at(base, k)));
        }
        while self.ups.len() + 1 < deepest as usize {
            let k = self.ups.len() as u32 + 1;
            self.ups.push(level(device, format, size_at(base, k)));
        }
        self.results.retain(|n, _| levels.contains(n));
        for &n in levels.iter().filter(|&&n| n > 0) {
            self.results
                .entry(n)
                .or_insert_with(|| level(device, format, base));
        }
    }
}

/// The blur's shared GPU state: its shader, layout and per-format pipelines.
pub(crate) struct BlurPipelines {
    shader: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    /// `(down, up)` per target format.
    pipelines: HashMap<wgpu::TextureFormat, (wgpu::RenderPipeline, wgpu::RenderPipeline)>,
}

impl BlurPipelines {
    pub(crate) fn new(device: &wgpu::Device, shader: wgpu::ShaderModule) -> Self {
        let layout = crate::render::gpu::bind_layouts::create_texture_layout(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("UI Blur Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        Self {
            shader,
            layout,
            pipeline_layout,
            pipelines: HashMap::new(),
        }
    }

    fn ensure(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        if self.pipelines.contains_key(&format) {
            return;
        }
        let build = |entry: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("UI Blur Pipeline"),
                layout: Some(&self.pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_full"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(format.into())],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let pair = (build("fs_down"), build("fs_up"));
        self.pipelines.insert(format, pair);
    }
}

/// Every blur level the overlay canvases' backdrops need this frame, ascending.
fn wanted_levels(view: &RenderView, divisor: u32) -> Vec<u32> {
    let mut levels: Vec<u32> = view
        .ui
        .canvases()
        .iter()
        .filter(|c| c.place.space == CanvasSpace::Screen)
        .flat_map(|c| c.mesh.batches.iter().filter_map(|b| b.backdrop))
        .map(|b| blur_level(b.radius, divisor))
        .collect();
    levels.sort_unstable();
    levels.dedup();
    levels
}

impl Renderer {
    /// Blur the finished frame at every level the overlay canvases' backdrops use
    /// (see the module docs). A no-op when none is visible.
    pub(crate) fn run_backdrop_blur(&mut self, view: &mut RenderView) {
        view.ui.effects.blur_passes = 0;
        let divisor = self.quality.backdrop_divisor();
        let levels = wanted_levels(view, divisor);
        let Some(&deepest) = levels.last() else {
            return;
        };
        let format = view.post_fx.output_format();
        let size = view.size();
        let base = size_at((size.width, size.height), divisor.trailing_zeros());
        let blur = &mut view.ui.effects.blur;
        if blur
            .as_ref()
            .is_none_or(|b| (b.format, b.base) != (format, base))
        {
            *blur = Some(BlurTargets::new(format, base));
        }
        let Some(targets) = blur.as_mut() else {
            return;
        };
        targets.ensure(&self.device, deepest, &levels);
        self.ui_renderer.blur.ensure(&self.device, format);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("UI Backdrop Encoder"),
            });
        let skybox = self.skybox_texture.as_ref().map(|t| &t.view);
        let ctx = PostFxContext {
            depth_view: &view.depth_view,
            skybox_view: skybox.unwrap_or(&self.default_texture.view),
            output: &targets.downs[0].view,
            timer: &self.gpu_timer,
        };
        view.post_fx.composite_into(&self.device, &mut encoder, ctx);
        let passes = self.encode_chain(&mut encoder, targets, deepest, &levels);
        self.queue.submit(Some(encoder.finish()));
        view.ui.effects.blur_passes = 1 + passes;
    }

    /// Record the down chain to `deepest` and each level's way back up; returns how
    /// many passes that was.
    fn encode_chain(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        t: &BlurTargets,
        deepest: u32,
        levels: &[u32],
    ) -> u32 {
        let pipes = &self.ui_renderer.blur;
        let Some((down, up)) = pipes.pipelines.get(&t.format) else {
            return 0;
        };
        let mut passes = 0;
        let mut run = |pipeline: &wgpu::RenderPipeline, src: &Level, dst: &wgpu::TextureView| {
            let group = crate::render::ui::bind(
                &self.device,
                &pipes.layout,
                &src.view,
                &self.ui_renderer.sampler,
            );
            fullscreen(encoder, (pipeline, &self.gpu_timer), &group, dst);
            passes += 1;
        };
        for k in 1..=deepest as usize {
            run(down, &t.downs[k - 1], &t.downs[k].view);
        }
        for &n in levels.iter().filter(|&&n| n > 0) {
            let mut src = &t.downs[n as usize];
            for k in (1..n as usize).rev() {
                run(up, src, &t.ups[k - 1].view);
                src = &t.ups[k - 1];
            }
            run(up, src, &t.results[&n].view);
        }
        passes
    }
}

/// One fullscreen-triangle pass of `pipeline` sampling `group` into `target`.
fn fullscreen(
    encoder: &mut wgpu::CommandEncoder,
    (pipeline, timer): (&wgpu::RenderPipeline, &GpuTimer),
    group: &wgpu::BindGroup,
    target: &wgpu::TextureView,
) {
    let clear = wgpu::LoadOp::Clear(wgpu::Color::BLACK);
    let mut pass = super::color_pass(encoder, ("UI Blur Pass", timer), target, clear);
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.draw(0..3, 0..1);
}
