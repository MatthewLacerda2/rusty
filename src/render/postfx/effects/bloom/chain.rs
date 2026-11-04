//! Bloom's mip chain (#1700), one view's: the down mips the prefilter and the
//! downsamples write, each half the one above, and the up mips the upsamples write
//! back towards the top. The composite reads the top of the up chain
//! ([`BloomTargets::view`]), the lens flare the prefilter's mip
//! ([`BloomTargets::prefiltered`]). A view holds one only while it blooms
//! (`PostFx::targets`), so a view that never blooms allocates none.

use super::passes::BloomPasses;
use crate::components::Bloom;
use crate::render::postfx::{PfxTarget, PostFx, HDR_FORMAT};

/// The most mips a chain has: the issue's 6–8 (Jimenez's 6, UE's 6, Godot's 7).
pub(crate) const MAX_MIPS: usize = 8;

/// How many mips a chain starting at `(w, h)` has, URP's
/// `CalcBloomMipCount`: `⌊log2(max side)⌋ − 1`, at least one and at most
/// [`MAX_MIPS`], so its last mip is a few texels across.
pub(crate) fn mip_count((w, h): (u32, u32)) -> usize {
    let side = w.max(h).max(1);
    let log2 = side.ilog2() as usize;
    log2.saturating_sub(1).clamp(1, MAX_MIPS)
}

/// The size of every mip of a chain starting at `start`, top first, each half the
/// one above (rounded down, never below one texel).
pub(crate) fn mip_sizes(start: (u32, u32)) -> Vec<(u32, u32)> {
    let mut size = start;
    let mut sizes = Vec::with_capacity(MAX_MIPS);
    for _ in 0..mip_count(start) {
        sizes.push(size);
        size = ((size.0 / 2).max(1), (size.1 / 2).max(1));
    }
    sizes
}

/// The largest divisor the prefilter writes at. Its 13 taps sit one target texel
/// apart, which reaches every scene pixel only up to `÷2`: straight from the scene
/// at `÷3` a one-pixel lamp at 4 of its 9 positions meets no tap, at `÷4` 8 of 16.
/// So past `÷2` it writes half the scene's size and a resample (the same 13 taps,
/// 1.5 or 2 texels apart) takes that to the chain's first mip (`prefilter_tests`).
pub(crate) const PREFILTER_DIVISOR: u32 = 2;

/// CPU mirror of `BloomParams` in `postfx/bloom.wgsl`.
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

/// The upsample's blend for a `scatter` of `0..=1`, URP's `lerp(0.05, 0.95,
/// scatter)`: how much of the mip below each mip takes.
pub(super) fn blend(scatter: f32) -> f32 {
    0.05 + 0.9 * scatter.clamp(0.0, 1.0)
}

impl BloomParams {
    /// The chain's params for `bloom`'s knobs and `t`'s chain: URP's soft knee
    /// fixed at half the threshold, its scatter mapping, and the steps of the
    /// prefilter and the resample.
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

/// One view's chain, at its quality's reduced resolution, with the bind groups of
/// every pass but the prefilter's (whose input, the scene, the frame picks).
pub(crate) struct BloomTargets {
    /// The prefilter's half-size target past `÷2`, which the resample reads.
    pub(super) pre: Option<PfxTarget>,
    /// `down[0]` is the prefilter's, at `size`; each after it half the one before.
    pub(super) down: Vec<PfxTarget>,
    /// `up[i]` is `down[i]` with everything below it blended in; one fewer than
    /// `down` (a one-mip chain's result is `down[0]`).
    pub(super) up: Vec<PfxTarget>,
    /// The downsamples' then the upsamples' groups, in the order they run.
    pub(super) groups: Vec<wgpu::BindGroup>,
    /// The first mip's size, the chain's result's.
    pub(crate) size: (u32, u32),
    /// The divisor `size` was made at; the bloom pass rebuilds the chain when the
    /// volume's quality asks for another.
    pub(crate) divisor: u32,
    pub(super) params: wgpu::Buffer,
}

impl BloomTargets {
    /// The chain for a view `scene` (width × height), starting at `1 / divisor`
    /// of it, its passes reading with `sampler`.
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
            divisor,
            params,
        }
    }

    /// The prefilter's target: the half-size image past `÷2`, else the first mip.
    pub(super) fn prefilter_target(&self) -> &PfxTarget {
        self.pre.as_ref().unwrap_or(&self.down[0])
    }

    /// The size of [`BloomTargets::prefilter_target`].
    pub(super) fn prefilter_size(&self) -> (u32, u32) {
        let t = &self.prefilter_target().texture;
        (t.width(), t.height())
    }

    /// The chain's result, which the composite adds: the top of the up chain, or
    /// the only mip.
    pub(crate) fn view(&self) -> &wgpu::TextureView {
        let top = self.up.first().unwrap_or(&self.down[0]);
        &top.view
    }

    /// The chain's first mip, thresholded and not yet spread: what the lens flare
    /// builds from, so neither the scatter nor the chain's width reshapes it.
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
