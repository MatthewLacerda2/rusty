//! A view's TAA history ([`TaaHistory`]) and the frame's start ([`begin`]): which
//! of TAA's two paths the view takes, and the jitter and mip bias it draws with.
//!
//! **Two paths, by the view's sizes** (#1702):
//! - **Resolve** (scene size equals output, or the scene is supersampled): the
//!   history is at the scene size and the resolve is a scene pass, after depth of
//!   field and before motion blur (URP's order). What TAA always did.
//! - **Upscale** (TAAU: the scene is smaller than the output, a render scale below
//!   1): the history is at the **output** size and the resolve runs after the post
//!   chain, in place of the bilinear upscale (`Renderer::taa_upscale`). The scene
//!   pass then skips, so nothing resolves or upscales twice. The jitter runs FSR2's
//!   longer sequence and the material maps read with a negative mip bias.

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

/// One view's TAA state while it runs: the two history targets, where the jitter
/// sequence is and whether the history holds a resolved frame.
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
    pub(super) params: wgpu::Buffer,
    /// Which of `frames` holds the latest resolve.
    pub(super) latest: usize,
    /// Whether `frames[latest]` is a frame of this view (not a fresh target).
    pub(super) valid: bool,
    /// The jitter sequence's frame counter.
    pub(super) frame: u32,
    /// Whether the scene-size resolve ran this frame: its output is then the scene.
    /// Never set while upscaling, whose output is past the post chain.
    pub(super) ran: bool,
}

impl TaaHistory {
    fn new(device: &wgpu::Device, (w, h): (u32, u32), upscale: bool, phases: u32) -> Self {
        let frame = |label| PostFx::target(device, w, h, label, HDR_FORMAT);
        Self {
            size: (w, h),
            upscale,
            phases,
            pixels: Vec2::ZERO,
            frames: [frame("TAA History A"), frame("TAA History B")],
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
        }
    }

    /// This frame's scene-size resolve, when it ran: what the rest of the chain reads.
    pub(crate) fn output(&self) -> Option<&PfxTarget> {
        self.ran.then(|| &self.frames[self.latest])
    }
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
