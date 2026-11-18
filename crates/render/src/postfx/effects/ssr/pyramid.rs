//! SSR's per-view textures besides the params: the copy of the opaque image the
//! trace reads its hits from and the resolve draws over, one level ([`colour_copy`]),
//! and the trace's two pyramids the resolve reads ([`HitPyramids`], #2028): its hits,
//! and where it traced and how wide its hits spread (#2158), both [`HDR_FORMAT`].
//!
//! The trace reads its hit colour sharp (`fs_trace` passes `trace_hit` a
//! `blur_scale` of 0, so its lod is 0): the resolve does the blurring, from the hit
//! pyramids, so the copy needs no mips.
//!
//! Each hit pyramid's texel is the average of the finite texels among the 2×2 above
//! it (`postfx/ssr_pyramid.wgsl`; an odd level's last row or column is left out of
//! the one below, as a mip chain's floor halving leaves no texel for it). Both are
//! built together, one fullscreen pass per level into both as two colour targets,
//! and only as deep as the resolve reads ([`hit_levels`]): at 1080p nine levels,
//! eight passes, about 22 MB each. They are render targets only, never storage: a
//! storage-capable colour image loses framebuffer compression on most desktop GPUs,
//! and the trace's full-size writes and the resolve's reads of level 0 dwarf the
//! mips'. Measured on a desktop GPU against one compute pass over storage
//! pyramids, this render chain is cheaper both to build and to trace into.
//!
//! All of it is the view's, held only while it runs SSR and remade at a new size
//! ([`super::targets::SsrTargets`]).

use crate::gpu::bind_entry as entry;
use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::layout_entries::texture_entry;
use crate::gpu::ops::{CountedDevice as _, CountedEncoder as _};
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::postfx::HDR_FORMAT;
use crate::timing::{GpuPass, GpuTimer};

/// How many levels a full `(w, h)` mip chain has: down to 1×1.
pub(crate) fn levels((w, h): (u32, u32)) -> u32 {
    u32::BITS - w.max(h).max(1).leading_zeros()
}

/// Mirrors `BLUR_FADE_END` in `lighting/ssr_march.wgsl`: the widest a resolve's
/// blur radius gets, as a share of the screen's shorter side (`ssr.wgsl`'s
/// `spread` caps it there).
pub(crate) const BLUR_FADE_END: f32 = 0.12;

/// Headroom over the deepest lod the shader computes, so a GPU's `log2` a few ulps
/// above the CPU's never reads past the last level.
const LOD_SLACK: f32 = 1.0 + 1.0 / 64.0;

/// How many levels the hit pyramids of a `(w, h)` view hold: as deep as the
/// resolve reads. Its deepest read is `blur_radius`' first, the spread pyramid at
/// `log2(cap)`, `cap` at most [`BLUR_FADE_END`] of the shorter side (the hits are
/// read shallower, at `log2(radius / 2)`); a trilinear read there takes the level
/// below it too. Never deeper than the full chain.
pub(crate) fn hit_levels((w, h): (u32, u32)) -> u32 {
    let cap = BLUR_FADE_END * w.min(h).max(1) as f32;
    let deepest = (cap * LOD_SLACK).log2().ceil().max(0.0) as u32;
    (deepest + 1).min(levels((w, h)))
}

/// The downsample pipeline and its layout, and the trilinear sampler the trace and
/// the resolve read through, shared by every view.
pub(crate) struct PyramidPass {
    layout: wgpu::BindGroupLayout,
    downsample: LazyPipeline,
    /// Linear between texels and between levels, clamped at the screen's edge.
    pub(crate) trilinear: wgpu::Sampler,
}

impl PyramidPass {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "postfx/ssr_pyramid.wgsl", "SSR Pyramid Shader");
        let d2 = wgpu::TextureViewDimension::D2;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSR Pyramid Layout"),
            entries: &[texture_entry(0, d2), texture_entry(1, d2)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSR Pyramid Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let downsample = LazyPipeline::new(device, move |device| {
            let target = wgpu::ColorTargetState {
                format: HDR_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            };
            Fullscreen {
                label: "SSR Pyramid Pipeline",
                shader: &shader,
                layout: &pipeline_layout,
                vs: "vs_fullscreen",
                fs: "fs_downsample",
            }
            .colour(device, &[Some(target.clone()), Some(target)])
        });
        let trilinear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SSR Pyramid Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        Self {
            layout,
            downsample,
            trilinear,
        }
    }

    pub(crate) fn lazy_pipeline(&self) -> &LazyPipeline {
        &self.downsample
    }
}

/// An [`HDR_FORMAT`] texture of `size` and `levels`, labelled `label`.
fn texture(
    device: &wgpu::Device,
    label: &str,
    (width, height): (u32, u32),
    levels: u32,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage,
        view_formats: &[],
    })
}

/// The one-level copy of the opaque image the trace and the resolve read.
pub(crate) fn colour_copy(device: &wgpu::Device, size: (u32, u32)) -> wgpu::Texture {
    use wgpu::TextureUsages as U;
    texture(
        device,
        "SSR Colour",
        size,
        1,
        U::TEXTURE_BINDING | U::COPY_DST,
    )
}

/// One hit pyramid: a view of every level (what the resolve reads) and one of each.
struct Chain {
    view: wgpu::TextureView,
    levels: Vec<wgpu::TextureView>,
}

impl Chain {
    fn new(device: &wgpu::Device, label: &str, size: (u32, u32), count: u32) -> Self {
        use wgpu::TextureUsages as U;
        // The trace draws the first level, the downsample the rest.
        let usage = U::RENDER_ATTACHMENT | U::TEXTURE_BINDING;
        let texture = texture(device, label, size, count, usage);
        let levels = (0..count)
            .map(|mip| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: mip,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        Self {
            view: texture.create_view(&Default::default()),
            levels,
        }
    }
}

/// One view's two hit pyramids, the same size and depth, and for each level below
/// the first the group that reads both levels above it.
pub(crate) struct HitPyramids {
    hits: Chain,
    spread: Chain,
    steps: Vec<wgpu::BindGroup>,
}

impl HitPyramids {
    pub(crate) fn new(device: &wgpu::Device, pass: &PyramidPass, size: (u32, u32)) -> Self {
        let count = hit_levels(size);
        let hits = Chain::new(device, "SSR Hits", size, count);
        let spread = Chain::new(device, "SSR Spread", size, count);
        let view = |v| wgpu::BindingResource::TextureView(v);
        let steps = (1..count as usize)
            .map(|mip| {
                device.counted_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("SSR Pyramid Level"),
                    layout: &pass.layout,
                    entries: &[
                        entry(0, view(&hits.levels[mip - 1])),
                        entry(1, view(&spread.levels[mip - 1])),
                    ],
                })
            })
            .collect();
        Self {
            hits,
            spread,
            steps,
        }
    }

    /// The first levels, the trace's targets: its hits, its spread.
    pub(crate) fn tops(&self) -> [&wgpu::TextureView; 2] {
        [&self.hits.levels[0], &self.spread.levels[0]]
    }

    /// Every level of each, as the resolve reads them: its hits, its spread.
    pub(crate) fn views(&self) -> [&wgpu::TextureView; 2] {
        [&self.hits.view, &self.spread.view]
    }

    /// How many levels each holds.
    #[cfg(test)]
    pub(crate) fn levels(&self) -> u32 {
        self.hits.levels.len() as u32
    }

    /// Record every level below the first of both, from the one above, one pass
    /// per level into both, timed as one `ssr_pyramids` pair across them.
    pub(crate) fn build(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pass: &PyramidPass,
        timer: &GpuTimer,
    ) {
        // A pair is only taken for passes that will write it: none at one level.
        let Some(last) = self.steps.len().checked_sub(1) else {
            return;
        };
        let (mut open, mut close) = match last {
            0 => (timer.writes(GpuPass::SsrPyramids), None),
            _ => timer.render_span(GpuPass::SsrPyramids),
        };
        for (i, group) in self.steps.iter().enumerate() {
            let mip = i + 1;
            let targets = [&self.hits.levels[mip], &self.spread.levels[mip]].map(|view| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })
            });
            let stamps = if i == 0 { open.take() } else { None };
            let stamps = stamps.or_else(|| if i == last { close.take() } else { None });
            let mut rp = encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SSR Pyramid Pass"),
                color_attachments: &targets,
                depth_stencil_attachment: None,
                timestamp_writes: stamps,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(pass.downsample.get());
            rp.set_bind_group(0, group, &[]);
            rp.draw(0..3, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{hit_levels, levels, BLUR_FADE_END};

    #[test]
    fn blur_fade_end_mirrors_the_shader() {
        let wgsl = include_str!("../../../../../../engine/shaders/lighting/ssr_march.wgsl");
        let line = format!("const BLUR_FADE_END: f32 = {BLUR_FADE_END};");
        assert!(wgsl.contains(&line), "ssr_march.wgsl must declare `{line}`");
    }

    #[test]
    fn the_hit_pyramids_stop_at_the_deepest_level_the_resolve_reads() {
        // 1080p: the cap is 129.6 px, lod 7.02, read between levels 7 and 8.
        assert_eq!(hit_levels((1920, 1080)), 9);
        // 40×30: 3.6 px, lod 1.85, levels 1 and 2.
        assert_eq!(hit_levels((40, 30)), 3);
        assert_eq!(hit_levels((96, 72)), 5);
        // Never deeper than the chain, and never empty.
        assert_eq!(hit_levels((1, 1)), 1);
        assert_eq!(hit_levels((0, 0)), 1);
        for size in [(4000, 4000), (64, 2), (2, 64)] {
            assert!(hit_levels(size) <= levels(size));
        }
    }
}
