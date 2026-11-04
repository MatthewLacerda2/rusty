//! SSIL's pipelines (#1750), one per [`Stage`] of `postfx/ssil.wgsl`, shared by every
//! view, built on first use and warmed with the rest; and how one stage records.

use crate::render::gpu::bind_entry as entry;
use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{depth_entry, texture_entry, uniform_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::HDR_FORMAT;
use crate::render::timing::{GpuPass, GpuTimer};

/// One of SSIL's fullscreen passes, in the order a view records them over two
/// frames: the source after the opaque pass, the rest before the next one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Source,
    Gather,
    Blur,
    Accumulate,
    Upsample,
}

impl Stage {
    const ALL: [Self; 5] = [
        Self::Source,
        Self::Gather,
        Self::Blur,
        Self::Accumulate,
        Self::Upsample,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Source => "SSIL Source Pass",
            Self::Gather => "SSIL Gather Pass",
            Self::Blur => "SSIL Blur Pass",
            Self::Accumulate => "SSIL Accumulate Pass",
            Self::Upsample => "SSIL Upsample Pass",
        }
    }

    /// Its fragment entry point in `postfx/ssil.wgsl`.
    fn entry(self) -> &'static str {
        match self {
            Self::Source => "fs_source",
            Self::Gather => "fs_gather",
            Self::Blur => "fs_blur",
            Self::Accumulate => "fs_accumulate",
            Self::Upsample => "fs_upsample",
        }
    }

    /// The formats it writes: the accumulate keeps each texel's normal beside its
    /// result, for next frame's rejection.
    fn targets(self) -> &'static [Option<wgpu::ColorTargetState>] {
        const fn plain(format: wgpu::TextureFormat) -> Option<wgpu::ColorTargetState> {
            Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })
        }
        const ONE: &[Option<wgpu::ColorTargetState>] = &[plain(HDR_FORMAT)];
        const TWO: &[Option<wgpu::ColorTargetState>] = &[plain(HDR_FORMAT), plain(NORMAL_FORMAT)];
        match self {
            Self::Accumulate => TWO,
            _ => ONE,
        }
    }
}

/// The format of the normals the accumulate keeps (`n * 0.5 + 0.5`).
pub(super) const NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// What one stage reads: the view's uniform and depth, its input, last frame's
/// result and its normals (its input again, for every stage but the accumulate).
pub(super) struct StageInputs<'a> {
    pub uniform: &'a wgpu::Buffer,
    pub depth: &'a wgpu::TextureView,
    pub input: &'a wgpu::TextureView,
    pub prev: &'a wgpu::TextureView,
    pub prev_normal: &'a wgpu::TextureView,
}

/// The renderer-wide half of SSIL: its layout and pipelines.
pub(crate) struct SsilPasses {
    layout: wgpu::BindGroupLayout,
    pipelines: [LazyPipeline; 5],
}

impl SsilPasses {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "postfx/ssil.wgsl", "SSIL Shader");
        let d2 = wgpu::TextureViewDimension::D2;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSIL Layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                depth_entry(1, d2),
                texture_entry(2, d2),
                texture_entry(3, d2),
                texture_entry(4, d2),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSIL Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |stage: Stage| {
            let (shader, layout) = (shader.clone(), pipeline_layout.clone());
            LazyPipeline::new(device, move |device| {
                let pass = Fullscreen {
                    label: stage.label(),
                    shader: &shader,
                    layout: &layout,
                    vs: "vs_fullscreen",
                    fs: stage.entry(),
                };
                pass.colour(device, stage.targets())
            })
        };
        Self {
            layout,
            pipelines: Stage::ALL.map(pipeline),
        }
    }

    /// The pipelines, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.pipelines.iter()
    }

    /// Record `stage` over `inputs` into `targets` (the accumulate's two, every
    /// other stage's one), timed under `ssil`.
    pub(super) fn record(
        &self,
        (device, encoder): (&wgpu::Device, &mut wgpu::CommandEncoder),
        stage: Stage,
        inputs: StageInputs,
        (targets, timer): (&[&wgpu::TextureView], &GpuTimer),
    ) {
        let view = wgpu::BindingResource::TextureView;
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSIL Bind Group"),
            layout: &self.layout,
            entries: &[
                entry(0, inputs.uniform.as_entire_binding()),
                entry(1, view(inputs.depth)),
                entry(2, view(inputs.input)),
                entry(3, view(inputs.prev)),
                entry(4, view(inputs.prev_normal)),
            ],
        });
        let attachments: Vec<_> = (targets.iter())
            .map(|&view| {
                Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })
            })
            .collect();
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            multiview_mask: None,
            label: Some(stage.label()),
            color_attachments: &attachments,
            depth_stencil_attachment: None,
            timestamp_writes: timer.writes(GpuPass::Ssil),
            occlusion_query_set: None,
        });
        pass.set_pipeline(self.pipelines[stage as usize].get());
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
}
