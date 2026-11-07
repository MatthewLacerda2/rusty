//! FSR2's per-view resources ([`Fsr2Targets`], in `PostFx::targets` while the view
//! runs FSR2): FSR2's internal surfaces at its formats, or the nearest a core
//! WebGPU storage texture takes, and the read-write images core WGSL lacks as
//! storage buffers.
//!
//! **Memory, per FSR2 camera.** At the display's size: two upscaled colour frames
//! (8 bytes a pixel each), two lock statuses (8 each, FSR2's `rg16f` widened to
//! `rgba16f`), two luma histories (4 each), the output (8) and the new locks (4):
//! 52 bytes a pixel, 108 MB at 1920×1080. At the render's: the prepared colour (8),
//! two dilated motions (8 each), the dilated depth, lock luma and reconstructed
//! depth (4 each) and the dilated reactive masks (4): 40 bytes a pixel, 21 MB at
//! 960×540. The level's size changes none of it.

use wgpu::util::DeviceExt;

use crate::render::postfx::PfxTarget;

/// What the reconstructed previous depth is cleared to: the far plane (the
/// velocity target's sky, `FAR_DEPTH` in `postfx/fsr2/common.wgsl`), as bits.
const FAR_BITS: u32 = 0x477f_e000; // 65504.0

/// One view's FSR2 state while it runs FSR2.
pub(crate) struct Fsr2Targets {
    /// The render (scene) size.
    pub(super) render: (u32, u32),
    /// The display (output) size.
    pub(super) display: (u32, u32),
    pub(super) luma_mip: PfxTarget,
    /// The reconstructed previous nearest depth, one `u32` (an `f32`'s bits) per
    /// render pixel: FSR2's `r32ui` atomic-min image.
    pub(super) prev_depth: wgpu::Buffer,
    pub(super) dilated_depth: PfxTarget,
    /// This frame's dilated motion and last frame's, by [`Self::current`].
    pub(super) dilated_motion: [PfxTarget; 2],
    pub(super) lock_luma: PfxTarget,
    pub(super) prepared: PfxTarget,
    pub(super) reactive_masks: PfxTarget,
    /// One `u32` per display pixel, 1 where a thin feature locked it this frame:
    /// FSR2's `r8` new-locks image.
    pub(super) new_locks: wgpu::Buffer,
    pub(super) history: [PfxTarget; 2],
    pub(super) lock_status: [PfxTarget; 2],
    pub(super) luma_history: [PfxTarget; 2],
    /// The upscaled image: what the rest of the chain reads.
    pub(super) output: PfxTarget,
    pub(super) params: wgpu::Buffer,
    /// Which of each ping-pong pair this frame writes; the other holds last frame's.
    pub(super) current: usize,
    /// FSR2's frame index: 0 on the first frame and after a cut (a reset).
    pub(super) frame: u32,
    /// Whether FSR2 ran this frame: [`Self::output`] is then the chain's input.
    pub(crate) ran: bool,
}

impl Fsr2Targets {
    /// The targets for a `render`-sized frame upscaled to `display`.
    pub(crate) fn new(device: &wgpu::Device, render: (u32, u32), display: (u32, u32)) -> Self {
        use wgpu::TextureFormat::{R32Float, Rgba16Float, Rgba8Unorm};
        let at = |size: (u32, u32), format, label: &str| image(device, size, format, label);
        let pair =
            |size, format, label: &str| [0, 1].map(|i| at(size, format, &format!("{label} {i}")));
        let luma = ((render.0 / 32).max(1), (render.1 / 32).max(1));
        let pixels = |(w, h): (u32, u32)| (w * h) as usize;
        Self {
            render,
            display,
            luma_mip: at(luma, Rgba16Float, "FSR2 Luma Mip"),
            prev_depth: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("FSR2 Reconstructed Previous Depth"),
                contents: bytemuck::cast_slice(&vec![FAR_BITS; pixels(render)]),
                usage: wgpu::BufferUsages::STORAGE,
            }),
            dilated_depth: at(render, R32Float, "FSR2 Dilated Depth"),
            dilated_motion: pair(render, Rgba16Float, "FSR2 Dilated Motion"),
            lock_luma: at(render, R32Float, "FSR2 Lock Luma"),
            prepared: at(render, Rgba16Float, "FSR2 Prepared Colour"),
            reactive_masks: at(render, Rgba8Unorm, "FSR2 Dilated Reactive"),
            new_locks: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("FSR2 New Locks"),
                size: (pixels(display) * 4) as u64,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            }),
            history: pair(display, Rgba16Float, "FSR2 Upscaled"),
            lock_status: pair(display, Rgba16Float, "FSR2 Lock Status"),
            luma_history: pair(display, Rgba8Unorm, "FSR2 Luma History"),
            output: at(display, Rgba16Float, "FSR2 Output"),
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("FSR2 Params"),
                size: std::mem::size_of::<super::params::Fsr2Params>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            current: 0,
            frame: 0,
            ran: false,
        }
    }

    /// Whether these targets fit a `render`-sized frame upscaled to `display`.
    pub(crate) fn fits(&self, render: (u32, u32), display: (u32, u32)) -> bool {
        (self.render, self.display) == (render, display)
    }

    /// Forget the history: the next frame is a reset (FSR2's frame index 0).
    pub(crate) fn reset(&mut self) {
        self.frame = 0;
    }

    /// This frame's upscaled image, when FSR2 ran: what the rest of the chain reads.
    pub(crate) fn output(&self) -> Option<&PfxTarget> {
        self.ran.then_some(&self.output)
    }
}

/// A `format` image at `size` the passes sample and write as storage.
fn image(
    device: &wgpu::Device,
    (w, h): (u32, u32),
    format: wgpu::TextureFormat,
    label: &str,
) -> PfxTarget {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        // COPY_SRC: a test reads the output back.
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    PfxTarget { texture, view }
}
