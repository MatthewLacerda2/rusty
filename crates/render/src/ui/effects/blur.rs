use std::collections::HashMap;

use super::backdrop::blur_level;
use crate::gpu::fullscreen::{color_pass, Fullscreen};
use crate::postfx::PostFxContext;
use crate::timing::{GpuPass, GpuTimer};
use crate::{RenderView, Renderer};
use rusty_ui::CanvasSpace;

/// `view` is sRGB, so blur taps filter in linear light; the UI pass reads `raw`, display-encoded.
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

fn size_at((w, h): (u32, u32), k: u32) -> (u32, u32) {
    ((w >> k).max(1), (h >> k).max(1))
}

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

    pub(crate) fn result(&self, level: u32) -> Option<&wgpu::TextureView> {
        match level {
            0 => self.downs.first().map(|l| &l.raw),
            n => self.results.get(&n).map(|l| &l.raw),
        }
    }

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

pub(crate) struct BlurPipelines {
    shader: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    pipelines: HashMap<wgpu::TextureFormat, (wgpu::RenderPipeline, wgpu::RenderPipeline)>,
}

impl BlurPipelines {
    pub(crate) fn new(device: &wgpu::Device, shader: wgpu::ShaderModule) -> Self {
        let layout = crate::gpu::bind_layouts::create_texture_layout(device);
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
            let pass = Fullscreen {
                label: "UI Blur Pipeline",
                shader: &self.shader,
                layout: &self.pipeline_layout,
                vs: "vs_full",
                fs: entry,
            };
            pass.colour(device, &[Some(format.into())])
        };
        let pair = (build("fs_down"), build("fs_up"));
        self.pipelines.insert(format, pair);
    }
}

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
    pub(crate) fn run_backdrop_blur(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &mut RenderView,
    ) {
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
        let sky = self.sky_panorama();
        let ctx = PostFxContext {
            depth_view: &view.depth_view,
            skybox: (&sky.view, &sky.sampler),
            output: &targets.downs[0].view,
            timer: &self.gpu_timer,
            passes: &self.post_passes,
        };
        view.post_fx.composite_into(&self.device, encoder, ctx);
        let passes = self.encode_chain(encoder, targets, deepest, &levels);
        view.ui.effects.blur_passes = 1 + passes;
    }

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
            let group = crate::ui::bind(
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

fn fullscreen(
    encoder: &mut wgpu::CommandEncoder,
    (pipeline, timer): (&wgpu::RenderPipeline, &GpuTimer),
    group: &wgpu::BindGroup,
    target: &wgpu::TextureView,
) {
    let clear = wgpu::LoadOp::Clear(wgpu::Color::BLACK);
    let mut pass = color_pass(encoder, ("UI Blur Pass", timer, GpuPass::Ui), target, clear);
    pass.set_pipeline(pipeline);
    pass.set_bind_group(crate::gpu::ONLY_GROUP, group, &[]);
    pass.draw(0..3, 0..1);
}
