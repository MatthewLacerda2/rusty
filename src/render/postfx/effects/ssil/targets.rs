//! One view's SSIL state (#1750), held in `PostFx::targets` while the view runs it:
//! the source (last frame's realtime light), the gather and its blur, the two
//! results the accumulate alternates between, the full-resolution result the
//! forward pass reads, and what the history was drawn from. Made at the view's
//! size and the quality's divisor, rebuilt (history and all) when either changes;
//! a resize drops it with every slot.

use super::{Previous, SsilUniform};
use crate::render::motion::HistoryKey;
use crate::render::postfx::{PfxTarget, PostFx, HDR_FORMAT};

/// What last frame left: its source and result, drawn from this camera for this key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct History {
    pub key: HistoryKey,
    pub camera: Previous,
}

pub(crate) struct SsilTargets {
    /// `(width, height, divisor)` these were made for.
    key: (u32, u32, u32),
    pub(super) source: PfxTarget,
    pub(super) raw: PfxTarget,
    pub(super) blurred: PfxTarget,
    pub(super) results: [PfxTarget; 2],
    /// The full-resolution result; `None` at full resolution, where the forward
    /// pass reads the current result itself.
    pub(super) full: Option<PfxTarget>,
    pub(super) uniform: wgpu::Buffer,
    /// Which of `results` this frame's accumulate writes.
    pub(super) current: usize,
    /// Frames run, modulo 64: turns the gather's noise.
    pub(super) index: u32,
    /// What the source and last result hold; `None` until the source is first kept,
    /// and after anything that invalidates them.
    pub(super) history: Option<History>,
    /// The forward pass's group 3 with this frame's result, while it runs this frame.
    pub(crate) forward_group: Option<wgpu::BindGroup>,
}

impl SsilTargets {
    /// Are these the targets for a `width` x `height` view at `divisor`?
    pub(super) fn matches(&self, (width, height): (u32, u32), divisor: u32) -> bool {
        self.key == (width, height, divisor)
    }

    pub(super) fn new(device: &wgpu::Device, (width, height): (u32, u32), divisor: u32) -> Self {
        let (w, h) = (width.div_ceil(divisor), height.div_ceil(divisor));
        let make = |label| PostFx::target(device, w, h, label, HDR_FORMAT);
        Self {
            key: (width, height, divisor),
            source: make("SSIL Source"),
            raw: make("SSIL Gather"),
            blurred: make("SSIL Blurred"),
            results: [make("SSIL Result A"), make("SSIL Result B")],
            full: (divisor > 1)
                .then(|| PostFx::target(device, width, height, "SSIL Full", HDR_FORMAT)),
            uniform: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("SSIL Uniform"),
                size: std::mem::size_of::<SsilUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            current: 0,
            index: 0,
            history: None,
            forward_group: None,
        }
    }

    /// The gather's resolution.
    pub(super) fn raw_size(&self) -> (u32, u32) {
        let size = self.raw.texture.size();
        (size.width, size.height)
    }

    /// What the forward pass reads: the full-resolution result, or the current one.
    pub(super) fn result(&self) -> &wgpu::TextureView {
        let current = &self.results[self.current].view;
        self.full.as_ref().map_or(current, |t| &t.view)
    }
}

#[cfg(test)]
impl PostFx {
    /// Whether the view holds SSIL's state (it ran this frame).
    pub(crate) fn holds_ssil(&self) -> bool {
        self.targets.get::<SsilTargets>().is_some()
    }

    /// Write NaN over the source's middle 8x8 texels, as a broken frame's light
    /// would leave them.
    pub(crate) fn poison_ssil_source(&self, queue: &wgpu::Queue) {
        let t = self.targets.get::<SsilTargets>().expect("ran");
        let size = t.source.texture.size();
        let nan = 0x7E00u16.to_le_bytes(); // a half-float NaN
        let bytes: Vec<u8> = nan.iter().copied().cycle().take(8 * 8 * 4 * 2).collect();
        let mut at = t.source.texture.as_image_copy();
        at.origin = wgpu::Origin3d {
            x: size.width / 2 - 4,
            y: size.height / 2 - 4,
            z: 0,
        };
        let layout = wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(8 * 8),
            rows_per_image: None,
        };
        let block = wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        };
        queue.write_texture(at, &bytes, layout, block);
    }

    /// The result the forward pass read this frame, decoded as stored (a NaN stays
    /// one).
    pub(crate) fn ssil_result(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Vec<glam::Vec3> {
        let t = self.targets.get::<SsilTargets>().expect("ran");
        let target = t.full.as_ref().unwrap_or(&t.results[t.current]);
        let size = target.texture.size();
        let read = crate::render::readback::read_texture_bytes;
        let bytes = read(device, queue, &target.texture, (size.width, size.height), 8);
        let half = |b: &[u8]| crate::core::color::f16_to_f32(u16::from_le_bytes([b[0], b[1]]));
        (bytes.chunks_exact(8))
            .map(|t| glam::Vec3::new(half(&t[..2]), half(&t[2..4]), half(&t[4..6])))
            .collect()
    }
}
