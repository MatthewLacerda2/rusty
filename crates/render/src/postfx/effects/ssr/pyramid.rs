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
//! built together, one compute dispatch per level, and only as deep as the resolve
//! reads ([`hit_levels`]): at 1080p nine levels, about 22 MB each.
//!
//! All of it is the view's, held only while it runs SSR and remade at a new size
//! ([`super::targets::SsrTargets`]).

use crate::gpu::bind_entry as entry;
use crate::gpu::ops::{CountedDevice as _, CountedEncoder as _};
use crate::gpu::pipelines::{lazy_compute, LazyCompute};
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

/// The downsample pipeline and the trilinear sampler the trace and the resolve
/// read through, shared by every view.
pub(crate) struct PyramidPass {
    downsample: LazyCompute,
    /// Linear between texels and between levels, clamped at the screen's edge.
    pub(crate) trilinear: wgpu::Sampler,
}

impl PyramidPass {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "postfx/ssr_pyramid.wgsl", "SSR Pyramid Shader");
        let names = ("SSR Pyramid Pipeline", "cs_downsample");
        let trilinear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SSR Pyramid Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        Self {
            downsample: lazy_compute(device, &shader, names, None),
            trilinear,
        }
    }

    pub(crate) fn lazy_compute(&self) -> &LazyCompute {
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
        // The trace draws the first level; the downsample stores the rest.
        let usage = U::RENDER_ATTACHMENT | U::STORAGE_BINDING | U::TEXTURE_BINDING;
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
/// the first the group that downsamples both into it.
pub(crate) struct HitPyramids {
    hits: Chain,
    spread: Chain,
    /// The full-size `(w, h)`.
    size: (u32, u32),
    steps: Vec<wgpu::BindGroup>,
}

impl HitPyramids {
    pub(crate) fn new(device: &wgpu::Device, pass: &PyramidPass, size: (u32, u32)) -> Self {
        let count = hit_levels(size);
        let hits = Chain::new(device, "SSR Hits", size, count);
        let spread = Chain::new(device, "SSR Spread", size, count);
        let layout = pass.downsample.get().get_bind_group_layout(0);
        let view = |v| wgpu::BindingResource::TextureView(v);
        let steps = (1..count as usize)
            .map(|mip| {
                device.counted_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("SSR Pyramid Level"),
                    layout: &layout,
                    entries: &[
                        entry(0, view(&hits.levels[mip - 1])),
                        entry(1, view(&spread.levels[mip - 1])),
                        entry(2, view(&hits.levels[mip])),
                        entry(3, view(&spread.levels[mip])),
                    ],
                })
            })
            .collect();
        Self {
            hits,
            spread,
            size,
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

    /// Record every level below the first of both, from the one above, in one
    /// compute pass, timed as `ssr_pyramids`.
    pub(crate) fn build(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pass: &PyramidPass,
        timer: &GpuTimer,
    ) {
        if self.steps.is_empty() {
            return;
        }
        let mut cp = encoder.counted_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("SSR Pyramids"),
            timestamp_writes: timer.compute_writes(GpuPass::SsrPyramids),
        });
        cp.set_pipeline(pass.downsample.get());
        let mut size = self.size;
        for group in &self.steps {
            size = ((size.0 / 2).max(1), (size.1 / 2).max(1));
            cp.set_bind_group(0, group, &[]);
            cp.dispatch_workgroups(size.0.div_ceil(8), size.1.div_ceil(8), 1);
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
