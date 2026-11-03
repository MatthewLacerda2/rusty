//! Bloom's mip chain (#1700), one view's: the down mips the prefilter and the
//! downsamples write, each half the one above, and the up mips the upsamples write
//! back towards the top. The composite and the lens flare read the top of the up
//! chain ([`BloomTargets::view`]).

use std::rc::Rc;

use crate::components::Bloom;
use crate::render::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::render::GpuTexture;

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

/// CPU mirror of `BloomParams` in `postfx/bloom.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct BloomParams {
    /// threshold, knee, the upsample's blend, the input's clamp
    pub(super) knobs: [f32; 4],
    /// one texel of the first mip (x, y), unused
    pub(super) texel: [f32; 4],
}

/// What `fs_prefilter` keeps of HDR input at most, URP's `ClampMax`: half floats
/// top out at 65504, so a brighter (or infinite) texel can't overflow the chain.
const CLAMP_MAX: f32 = 65472.0;

/// The upsample's blend for a `scatter` of `0..=1`, URP's `lerp(0.05, 0.95,
/// scatter)`: how much of the mip below each mip takes.
pub(super) fn blend(scatter: f32) -> f32 {
    0.05 + 0.9 * scatter.clamp(0.0, 1.0)
}

impl BloomParams {
    /// The chain's params for `bloom`'s knobs and a first mip of `size`: URP's
    /// soft knee fixed at half the threshold, and its scatter mapping.
    pub(super) fn new(bloom: &Bloom, size: (u32, u32)) -> Self {
        let threshold = bloom.threshold;
        let knee = threshold * 0.5;
        Self {
            knobs: [threshold, knee, blend(bloom.scatter), CLAMP_MAX],
            texel: [1.0 / size.0 as f32, 1.0 / size.1 as f32, 0.0, 0.0],
        }
    }
}

/// One view's chain, at its quality's reduced resolution, and the lens dirt it
/// lights (bloom's composite texture), picked by the bloom pass each frame.
pub(crate) struct BloomTargets {
    /// `down[0]` is the prefilter's, at `size`; each after it half the one before.
    pub(super) down: Vec<PfxTarget>,
    /// `up[i]` is `down[i]` with everything below it blended in; one fewer than
    /// `down` (a one-mip chain's result is `down[0]`).
    pub(super) up: Vec<PfxTarget>,
    /// The first mip's size, the chain's result's.
    pub(crate) size: (u32, u32),
    /// The divisor `size` was made at; the bloom pass rebuilds the chain when the
    /// volume's quality asks for another.
    pub(crate) divisor: u32,
    pub(super) params: wgpu::Buffer,
    pub(super) dirt: Option<Rc<GpuTexture>>,
}

impl BloomTargets {
    /// The divisor a view's chain starts at, before its bloom first runs: half.
    pub(crate) const DIVISOR: u32 = 2;

    /// The chain for a view `width` × `height`, starting at `1 / divisor` of it.
    pub(crate) fn new(device: &wgpu::Device, (w, h): (u32, u32), divisor: u32) -> Self {
        let size = ((w / divisor).max(1), (h / divisor).max(1));
        let sizes = mip_sizes(size);
        let target =
            |&(w, h): &(u32, u32), label: &str| PostFx::target(device, w, h, label, HDR_FORMAT);
        let down = sizes.iter().map(|s| target(s, "PostFX Bloom Down"));
        let up = sizes[..sizes.len() - 1].iter();
        Self {
            down: down.collect(),
            up: up.map(|s| target(s, "PostFX Bloom Up")).collect(),
            size,
            divisor,
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Bloom Params"),
                size: std::mem::size_of::<BloomParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            dirt: None,
        }
    }

    /// The chain's result, which the composite adds and the lens flare reads: the
    /// top of the up chain, or the only mip.
    pub(crate) fn view(&self) -> &wgpu::TextureView {
        let top = self.up.first().unwrap_or(&self.down[0]);
        &top.view
    }

    /// The lens dirt the composite reads this frame, if any (the registry binds
    /// black without one).
    pub(crate) fn dirt_view(&self) -> Option<&wgpu::TextureView> {
        self.dirt.as_ref().map(|t| &t.view)
    }
}

#[cfg(test)]
#[path = "chain_tests.rs"]
mod tests;
