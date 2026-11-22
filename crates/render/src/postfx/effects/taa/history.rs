use glam::Vec2;

use super::jitter;
use crate::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::RenderView;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TaaFrame {
    pub jitter: Vec2,
    pub mip_bias: f32,
}

pub(crate) const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

pub(crate) const FLICKER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

pub(crate) struct TaaHistory {
    pub(super) size: (u32, u32),
    pub(super) upscale: bool,
    pub(super) phases: u32,
    pub(super) pixels: Vec2,
    pub(super) frames: [PfxTarget; 2],
    pub(super) surfaces: [PfxTarget; 2],
    pub(super) flickers: Option<[PfxTarget; 2]>,
    pub(super) reactive: PfxTarget,
    pub(super) scene: (u32, u32),
    pub(super) reactive_drawn: std::sync::atomic::AtomicBool,
    pub(super) params: wgpu::Buffer,
    pub(super) latest: usize,
    pub(super) valid: bool,
    pub(super) frame: u32,
    pub(super) ran: bool,
    #[cfg(test)]
    pub(crate) rules: super::params::Rules,
}

impl TaaHistory {
    fn new(
        device: &wgpu::Device,
        [(w, h), scene]: [(u32, u32); 2],
        upscale: bool,
        phases: u32,
    ) -> Self {
        let frame = |label| PostFx::target(device, w, h, label, HDR_FORMAT);
        let surface = |label| PostFx::target(device, w, h, label, SURFACE_FORMAT);
        Self {
            size: (w, h),
            upscale,
            phases,
            pixels: Vec2::ZERO,
            frames: [frame("TAA History A"), frame("TAA History B")],
            surfaces: [surface("TAA Surfaces A"), surface("TAA Surfaces B")],
            flickers: None,
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

    pub(super) fn hold_flickers(&mut self, device: &wgpu::Device, on: bool) {
        match (on, self.flickers.is_some()) {
            (true, false) => {
                let (w, h) = self.size;
                let flicker = |label| PostFx::target(device, w, h, label, FLICKER_FORMAT);
                self.flickers = Some([flicker("TAA Flickers A"), flicker("TAA Flickers B")]);
            }
            (false, true) => self.flickers = None,
            _ => {}
        }
    }

    pub(crate) fn output(&self) -> Option<&PfxTarget> {
        self.ran.then(|| &self.frames[self.latest])
    }
}

pub(crate) fn chain_size(view: &RenderView, on: bool) -> (u32, u32) {
    let size = match upscales(view, on) {
        true => view.size(),
        false => view.scene_size(), // chain-size: scene the resolve's own input
    };
    (size.width, size.height)
}

pub(crate) fn upscales(view: &RenderView, on: bool) -> bool {
    // lint: allow(comment-width) a `chain-size` marker sits on the line it excuses
    on && jitter::upscales(view.scene_size(), view.size()) // chain-size: scene compared to the output
}

pub(crate) fn begin(
    view: &mut RenderView,
    device: &wgpu::Device,
    on: bool,
    (jitter_scale, mip_bias): (f32, f32),
) -> TaaFrame {
    let (scene, output) = (view.scene_size(), view.size()); // chain-size: scene the jitter's
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
    let scene_px = (scene.width.max(1), scene.height.max(1));
    let sizes = [size_px, scene_px];
    let h = targets.get_or(|| TaaHistory::new(device, sizes, upscale, phases));
    if (h.size, h.scene, h.upscale, h.phases) != (size_px, scene_px, upscale, phases) {
        *h = TaaHistory::new(device, sizes, upscale, phases);
    }
    if cut {
        (h.frame, h.valid) = (0, false);
    }
    h.frame = h.frame.wrapping_add(1);
    h.ran = false;
    h.pixels = jitter::pixels(h.frame, h.phases) * jitter_scale;
    TaaFrame {
        jitter: jitter::ndc(h.pixels, scene),
        mip_bias: jitter::mip_bias(scene, output) + mip_bias,
    }
}
