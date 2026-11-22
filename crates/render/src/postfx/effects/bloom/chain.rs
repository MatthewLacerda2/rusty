use super::passes::BloomPasses;
use crate::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use rusty_components::Bloom;

pub(crate) const MAX_MIPS: usize = 8;

/// URP's `CalcBloomMipCount`.
pub(crate) fn mip_count((w, h): (u32, u32)) -> usize {
    let side = w.max(h).max(1);
    let log2 = side.ilog2() as usize;
    log2.saturating_sub(1).clamp(1, MAX_MIPS)
}

pub(crate) fn mip_sizes(start: (u32, u32)) -> Vec<(u32, u32)> {
    let mut size = start;
    let mut sizes = Vec::with_capacity(MAX_MIPS);
    for _ in 0..mip_count(start) {
        sizes.push(size);
        size = ((size.0 / 2).max(1), (size.1 / 2).max(1));
    }
    sizes
}

/// Past `÷2` the 13 taps, a target texel apart, miss scene pixels: deeper divisors resample
/// this half-size target (`prefilter_tests`).
pub(crate) const PREFILTER_DIVISOR: u32 = 2;

/// Mirrors `BloomParams` in `postfx/bloom.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct BloomParams {
    /// threshold, knee, the upsample's blend, the input's clamp
    pub(super) knobs: [f32; 4],
    /// one texel of the prefilter's target (x, y) and of the first mip (z, w), in uv
    pub(super) texel: [f32; 4],
}

/// What `fs_prefilter` keeps of HDR input at most, URP's `ClampMax`: half floats
/// top out at 65504, so a brighter (or infinite) texel can't overflow the chain.
pub(super) const CLAMP_MAX: f32 = 65472.0;

pub(super) fn blend(scatter: f32) -> f32 {
    0.05 + 0.9 * scatter.clamp(0.0, 1.0)
}

impl BloomParams {
    pub(super) fn new(bloom: &Bloom, t: &BloomTargets) -> Self {
        let threshold = bloom.threshold;
        let knee = threshold * 0.5;
        let (pre, first) = (t.prefilter_size(), t.size);
        Self {
            knobs: [threshold, knee, blend(bloom.scatter), CLAMP_MAX],
            texel: [
                1.0 / pre.0 as f32,
                1.0 / pre.1 as f32,
                1.0 / first.0 as f32,
                1.0 / first.1 as f32,
            ],
        }
    }
}

pub(crate) struct BloomTargets {
    pub(super) pre: Option<PfxTarget>,
    pub(super) down: Vec<PfxTarget>,
    pub(super) up: Vec<PfxTarget>,
    pub(super) groups: Vec<wgpu::BindGroup>,
    pub(crate) size: (u32, u32),
    pub(crate) made_for: ((u32, u32), u32),
    pub(super) params: wgpu::Buffer,
}

impl BloomTargets {
    pub(crate) fn new(
        device: &wgpu::Device,
        passes: &BloomPasses,
        sampler: &wgpu::Sampler,
        scene: (u32, u32),
        divisor: u32,
    ) -> Self {
        let (w, h) = scene;
        let size = ((w / divisor).max(1), (h / divisor).max(1));
        let sizes = mip_sizes(size);
        let target =
            |&(w, h): &(u32, u32), label: &str| PostFx::target(device, w, h, label, HDR_FORMAT);
        let down: Vec<_> = sizes
            .iter()
            .map(|s| target(s, "PostFX Bloom Down"))
            .collect();
        let up = sizes[..sizes.len() - 1].iter();
        let up: Vec<_> = up.map(|s| target(s, "PostFX Bloom Up")).collect();
        let pre = (divisor > PREFILTER_DIVISOR).then(|| {
            let half = |side: u32| (side / PREFILTER_DIVISOR).max(1);
            target(&(half(w), half(h)), "PostFX Bloom Prefilter")
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bloom Params"),
            size: std::mem::size_of::<BloomParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mips = (pre.as_ref(), &down[..], &up[..]);
        let groups = passes.chain_groups(device, mips, &params, sampler);
        Self {
            pre,
            down,
            up,
            groups,
            size,
            made_for: (scene, divisor),
            params,
        }
    }

    pub(super) fn prefilter_target(&self) -> &PfxTarget {
        self.pre.as_ref().unwrap_or(&self.down[0])
    }

    pub(super) fn prefilter_size(&self) -> (u32, u32) {
        let t = &self.prefilter_target().texture;
        (t.width(), t.height())
    }

    pub(crate) fn view(&self) -> &wgpu::TextureView {
        let top = self.up.first().unwrap_or(&self.down[0]);
        &top.view
    }

    /// The first mip, thresholded and unspread: the lens flare reads it so that the scatter
    /// never reshapes the flare.
    pub(crate) fn prefiltered(&self) -> &wgpu::TextureView {
        &self.down[0].view
    }
}

#[cfg(test)]
#[path = "prefilter_tests.rs"]
mod prefilter_tests;
#[cfg(test)]
#[path = "chain_tests.rs"]
mod tests;
