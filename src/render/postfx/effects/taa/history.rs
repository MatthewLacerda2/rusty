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
//!
//! **FSR2 in TAA's place** (#1848): under the `fsr2` upscaler a screen view runs
//! FSR2 instead of either path ([`mode`]): this history then keeps the jitter and
//! the reactive mask and no TAA frames, and FSR2's own targets
//! ([`super::fsr2::Fsr2Targets`]) sit beside it at the output's size; the chain
//! runs at the output's size, as under TAAU, and the jitter and mip bias are FSR2's.
//!
//! On every path the camera's *Mip Bias* (#1847) adds to the material maps' bias.

use glam::Vec2;

use super::fsr2::{self, Fsr2Targets};
use super::jitter;
use crate::core::settings::upscaler::Upscaler;
use crate::render::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::render::RenderView;

/// What a stack draws with this frame under TAA: zero for both without it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TaaFrame {
    /// The projection's sub-pixel jitter, in NDC.
    pub jitter: Vec2,
    /// The material maps' mip bias: TAAU's ([`jitter::mip_bias`], negative only
    /// while the view upscales) plus the camera's *Mip Bias* (#1847), which applies
    /// on every frame TAA runs.
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
/// 3840×2160; and at the scene's size the reactive mask (2 bytes a pixel, #1845):
/// 4 MB at 1920×1080. The level's size (a Dust2-sized map or a room) changes none of it.
pub(crate) struct TaaHistory {
    /// The history's size: the scene's when resolving, the output's when upscaling.
    pub(super) size: (u32, u32),
    /// Whether this view upscales (TAAU) rather than resolving at the scene size.
    pub(super) upscale: bool,
    /// What resolves the frame: TAA's resolve, or FSR2 in its place (#1848).
    pub(super) resolver: Upscaler,
    /// The jitter sequence's length ([`jitter::phases`], or FSR2's
    /// [`fsr2::params::phases`]).
    pub(super) phases: u32,
    /// This frame's jitter in scene pixels.
    pub(super) pixels: Vec2,
    /// TAA's resolved frames; `None` while FSR2 resolves in TAA's place.
    pub(super) frames: Option<TaaFrames>,
    /// The reactive mask the translucents write this frame, at the scene's size
    /// ([`super::reactive`], #1845); cleared after each resolve.
    pub(super) reactive: PfxTarget,
    /// The scene's size, which [`Self::reactive`] has.
    pub(super) scene: (u32, u32),
    /// Whether a translucent pass bound [`Self::reactive`] since the last resolve
    /// (`reactive::hdr_attachments` sets it): when not, the mask is all zero, and
    /// the resolve skips reading it and clearing it.
    pub(super) reactive_drawn: std::sync::atomic::AtomicBool,
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

/// TAA's two history frames, read and written in turn.
pub(crate) struct TaaFrames {
    pub(super) colour: [PfxTarget; 2],
    /// Beside each of `colour`, each pixel's surface as that resolve saw it
    /// ([`SURFACE_FORMAT`]): next frame drops the history where this frame's nearest
    /// surface in the 3×3 (`Front::near`, not the pixel's own) lies farther than all
    /// of last frame's around the reprojected place, a reveal (#1846). Made, dropped
    /// and reset with `colour`, so it follows every cut, resize and render-scale
    /// change they do.
    pub(super) surfaces: [PfxTarget; 2],
}

impl TaaFrames {
    fn new(device: &wgpu::Device, (w, h): (u32, u32)) -> Self {
        let frame = |label| PostFx::target(device, w, h, label, HDR_FORMAT);
        let surface = |label| PostFx::target(device, w, h, label, SURFACE_FORMAT);
        Self {
            colour: [frame("TAA History A"), frame("TAA History B")],
            surfaces: [surface("TAA Surfaces A"), surface("TAA Surfaces B")],
        }
    }
}

/// What [`TaaHistory`] is made for: its size and the scene's, whether it upscales,
/// its phase count and its resolver.
#[derive(Clone, Copy, PartialEq)]
struct Shape {
    size: (u32, u32),
    scene: (u32, u32),
    upscale: bool,
    phases: u32,
    resolver: Upscaler,
}

impl TaaHistory {
    fn new(device: &wgpu::Device, shape: Shape) -> Self {
        let taa = shape.resolver == Upscaler::Taa;
        let scene = shape.scene;
        Self {
            size: shape.size,
            upscale: shape.upscale,
            resolver: shape.resolver,
            phases: shape.phases,
            pixels: Vec2::ZERO,
            frames: taa.then(|| TaaFrames::new(device, shape.size)),
            reactive: super::reactive::create(device, scene),
            scene,
            reactive_drawn: Default::default(),
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
        let frames = self.frames.as_ref().filter(|_| self.ran)?;
        Some(&frames.colour[self.latest])
    }

    fn shape(&self) -> Shape {
        Shape {
            size: self.size,
            scene: self.scene,
            upscale: self.upscale,
            phases: self.phases,
            resolver: self.resolver,
        }
    }
}

/// What resolves `view`'s frame when it runs TAA (`on`, [`super::runs`]) and the
/// settings name `upscaler` for it: FSR2 where they name it and the view does not
/// supersample (FSR2 only upscales), else TAA. `None` without TAA.
pub(crate) fn mode(view: &RenderView, on: bool, upscaler: Upscaler) -> Option<Upscaler> {
    let (scene, output) = (view.scene_size(), view.size()); // chain-size: scene the scale's
    resolver(on, upscaler, (scene, output))
}

/// [`mode`] on the view's sizes, for a unit test.
pub(crate) fn resolver(
    on: bool,
    upscaler: Upscaler,
    (scene, output): (crate::render::PixelSize, crate::render::PixelSize),
) -> Option<Upscaler> {
    let supersampled = scene.width > output.width || scene.height > output.height;
    on.then_some(match upscaler {
        Upscaler::Fsr2 if !supersampled => Upscaler::Fsr2,
        _ => Upscaler::Taa,
    })
}

/// The size the chain runs at after TAA this frame ([`PostFx::fit_chain`]): the
/// output's when the view upscales through TAA or FSR2 (`mode`, [`mode`]), else
/// the scene's.
pub(crate) fn chain_size(view: &RenderView, mode: Option<Upscaler>) -> (u32, u32) {
    let size = match upscales(view, mode) {
        true => view.size(),
        false => view.scene_size(), // chain-size: scene the resolve's own input
    };
    (size.width, size.height)
}

/// Whether `view`, resolved by `mode` ([`mode`]), writes the output's size this
/// frame: TAA upscaling it (TAAU, its scene smaller than its output), or FSR2 at
/// any scale it runs at (its output is the display's).
pub(crate) fn upscales(view: &RenderView, mode: Option<Upscaler>) -> bool {
    match mode {
        None => false,
        Some(Upscaler::Fsr2) => true,
        Some(Upscaler::Taa) => jitter::upscales(view.scene_size(), view.size()), // chain-size: scene compared to the output
    }
}

/// Start a stack's frame, after the motion history's `begin`: with a `mode`
/// ([`mode`]), fit the view's history (and FSR2's targets under FSR2) to its path
/// and size and return this frame's jitter and mip bias; without, drop both and
/// return zero for both. A view that runs no TAA draws with the unjittered
/// projection, bit for bit. On a cut (the motion history starts over) the history
/// is forgotten and the sequence restarts, so the frame is exactly a new view's
/// first. A resize or a render-scale change drops the history with every post
/// target (`PostFx::resize`): a cut too.
///
/// `knobs` is the base camera's `(jitter_scale, mip_bias)` (URP's *Jitter Scale* and
/// *Mip Bias*, #1847): the first scales the sub-pixel offset (`1` its default, so a
/// camera at its defaults jitters as before), the second adds the camera's negative
/// mip bias to the bias TAAU or FSR2 already applies.
pub(crate) fn begin(
    view: &mut RenderView,
    device: &wgpu::Device,
    mode: Option<Upscaler>,
    (jitter_scale, mip_bias): (f32, f32),
) -> TaaFrame {
    let (scene, output) = (view.scene_size(), view.size()); // chain-size: scene the jitter's
    let cut = view.post_fx.motion.history.cut();
    let targets = &mut view.post_fx.targets;
    if mode != Some(Upscaler::Fsr2) {
        targets.remove::<Fsr2Targets>();
    }
    let Some(resolver) = mode else {
        targets.remove::<TaaHistory>();
        return TaaFrame::default();
    };
    let fsr2 = resolver == Upscaler::Fsr2;
    let upscale = !fsr2 && jitter::upscales(scene, output);
    let fit = if upscale || fsr2 { output } else { scene };
    let (phases, bias) = match fsr2 {
        true => (
            fsr2::params::phases(scene, output),
            fsr2::params::mip_bias(scene, output),
        ),
        false => (
            jitter::phases(scene, output),
            jitter::mip_bias(scene, output),
        ),
    };
    let shape = Shape {
        size: (fit.width.max(1), fit.height.max(1)),
        scene: (scene.width.max(1), scene.height.max(1)),
        upscale,
        phases,
        resolver,
    };
    if fsr2 {
        let f = targets.get_or(|| Fsr2Targets::new(device, shape.scene, shape.size));
        if !f.fits(shape.scene, shape.size) {
            *f = Fsr2Targets::new(device, shape.scene, shape.size);
        }
        f.ran = false;
    }
    let h = targets.get_or(|| TaaHistory::new(device, shape));
    if h.shape() != shape {
        *h = TaaHistory::new(device, shape);
    }
    if cut {
        (h.frame, h.valid) = (0, false);
    }
    h.frame = h.frame.wrapping_add(1);
    h.ran = false;
    h.pixels = jitter::pixels(h.frame, h.phases) * jitter_scale;
    let (pixels, restarted) = (h.pixels, h.frame == 1);
    if let Some(f) = targets.get_mut::<Fsr2Targets>().filter(|_| restarted) {
        f.reset();
    }
    TaaFrame {
        jitter: jitter::ndc(pixels, scene),
        mip_bias: bias + mip_bias,
    }
}
