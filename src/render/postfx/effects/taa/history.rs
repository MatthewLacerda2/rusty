//! A view's TAA history ([`TaaHistory`]) and the frame's start ([`begin`]): which
//! of TAA's two paths the view takes, and the jitter and mip bias it draws with.
//!
//! **Two paths, by the view's sizes** (#1702), both a scene pass after depth of
//! field and before motion blur (URP's order):
//! - **Resolve** (scene size equals output, or the scene is supersampled): the
//!   history is at the scene size. What TAA always did.
//! - **Upscale** (TAAU: the scene is smaller than the output, a render scale below
//!   1): the history is at the **output** size and the resolve reconstructs it
//!   from the scene-size frame (`fs_upscale`), as URP's STP and HDRP's FSR2 sit.
//!   The rest of the chain then runs at the output's size ([`chain_size`]) and
//!   writes the output itself, so no bilinear upscale follows. The jitter runs
//!   FSR2's longer sequence and the material maps read with a negative mip bias.

use glam::Vec2;

use super::jitter;
use crate::render::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::render::RenderView;

/// What a stack draws with this frame under TAA: zero for both without it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TaaFrame {
    /// The projection's sub-pixel jitter, in NDC.
    pub jitter: Vec2,
    /// The material maps' mip bias ([`jitter::mip_bias`]): negative only while the
    /// view upscales.
    pub mip_bias: f32,
}

/// What [`TaaHistory::surfaces`] holds: two half floats a pixel, its own surface's
/// distance from its camera (the sky's 65504 fits) and that camera's index in the
/// stack (#1846).
pub(crate) const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

/// One view's TAA state while it runs: the two history targets, where the jitter
/// sequence is and whether the history holds a resolved frame.
///
/// **Memory, per TAA camera** at the history's size (the output's when
/// upscaling): two HDR colour frames (8 bytes a pixel each) and two surface
/// frames (4 bytes each, #1846), 24 bytes a pixel: 50 MB at 1920×1080, 199 MB at
/// 3840×2160. The level's size (a Dust2-sized map or a room) changes none of it.
pub(crate) struct TaaHistory {
    /// The history's size: the scene's when resolving, the output's when upscaling.
    pub(super) size: (u32, u32),
    /// Whether this view upscales (TAAU) rather than resolving at the scene size.
    pub(super) upscale: bool,
    /// The jitter sequence's length ([`jitter::phases`]).
    pub(super) phases: u32,
    /// This frame's jitter in scene pixels.
    pub(super) pixels: Vec2,
    pub(super) frames: [PfxTarget; 2],
    /// Beside each of `frames`, each pixel's surface as that resolve saw it
    /// ([`SURFACE_FORMAT`]): next frame drops the history where its own surface
    /// lies farther than all of last frame's around it, a reveal (#1846). Made, dropped and reset with `frames`, so
    /// it follows every cut, resize and render-scale change they do.
    pub(super) surfaces: [PfxTarget; 2],
    pub(super) params: wgpu::Buffer,
    /// Which of `frames` holds the latest resolve.
    pub(super) latest: usize,
    /// Whether `frames[latest]` is a frame of this view (not a fresh target).
    pub(super) valid: bool,
    /// The jitter sequence's frame counter.
    pub(super) frame: u32,
    /// Whether the resolve ran this frame: its output is then the image the rest of
    /// the chain reads, at the output's size when `upscale`.
    pub(super) ran: bool,
    /// Which of #1846's rules the resolve keeps: both, unless a test turned one off.
    #[cfg(test)]
    pub(crate) rules: super::params::Rules,
}

impl TaaHistory {
    fn new(device: &wgpu::Device, (w, h): (u32, u32), upscale: bool, phases: u32) -> Self {
        let frame = |label| PostFx::target(device, w, h, label, HDR_FORMAT);
        let surface = |label| PostFx::target(device, w, h, label, SURFACE_FORMAT);
        Self {
            size: (w, h),
            upscale,
            phases,
            pixels: Vec2::ZERO,
            frames: [frame("TAA History A"), frame("TAA History B")],
            surfaces: [surface("TAA Surfaces A"), surface("TAA Surfaces B")],
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("TAA Params"),
                size: std::mem::size_of::<super::TaaParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            latest: 0,
            valid: false,
            frame: 0,
            ran: false,
            #[cfg(test)]
            rules: Default::default(),
        }
    }

    /// This frame's resolve, when it ran: what the rest of the chain reads.
    pub(crate) fn output(&self) -> Option<&PfxTarget> {
        self.ran.then(|| &self.frames[self.latest])
    }
}

/// The size the chain runs at after TAA this frame ([`PostFx::fit_chain`]): the
/// output's when `on` and the view upscales through TAA, else the scene's.
pub(crate) fn chain_size(view: &RenderView, on: bool) -> (u32, u32) {
    let size = match upscales(view, on) {
        true => view.size(),
        false => view.scene_size(),
    };
    (size.width, size.height)
}

/// Whether `view`, running TAA when `on` ([`super::runs`]), upscales through it this
/// frame (TAAU): its scene is smaller than its output.
pub(crate) fn upscales(view: &RenderView, on: bool) -> bool {
    on && jitter::upscales(view.scene_size(), view.size())
}

/// Start a stack's frame, after the motion history's `begin`: with `on`, fit the
/// view's history to its path and size and return this frame's jitter and mip
/// bias; without, drop the history and return zero for both. A view that runs no
/// TAA draws with the unjittered projection, bit for bit. On a cut (the motion
/// history starts over) the history is forgotten and the sequence restarts, so the
/// frame is exactly a new view's first. A resize or a render-scale change drops the
/// history with every post target (`PostFx::resize`): a cut too.
pub(crate) fn begin(view: &mut RenderView, device: &wgpu::Device, on: bool) -> TaaFrame {
    let (scene, output) = (view.scene_size(), view.size());
    let cut = view.post_fx.motion.history.cut();
    let targets = &mut view.post_fx.targets;
    if !on {
        targets.remove::<TaaHistory>();
        return TaaFrame::default();
    }
    let upscale = jitter::upscales(scene, output);
    let fit = if upscale { output } else { scene };
    let size_px = (fit.width.max(1), fit.height.max(1));
    let phases = jitter::phases(scene, output);
    let h = targets.get_or(|| TaaHistory::new(device, size_px, upscale, phases));
    if (h.size, h.upscale, h.phases) != (size_px, upscale, phases) {
        *h = TaaHistory::new(device, size_px, upscale, phases);
    }
    if cut {
        (h.frame, h.valid) = (0, false);
    }
    h.frame = h.frame.wrapping_add(1);
    h.ran = false;
    h.pixels = jitter::pixels(h.frame, h.phases);
    TaaFrame {
        jitter: jitter::ndc(h.pixels, scene),
        mip_bias: jitter::mip_bias(scene, output),
    }
}
