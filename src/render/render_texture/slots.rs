//! src/render/render_texture/slots.rs — a view's render textures (#430): per name,
//! the offscreen view the camera draws into and the texture consumers sample.

use std::collections::HashMap;
use std::rc::Rc;

use crate::render::setup::headless::OFFSCREEN_FORMAT;
use crate::render::{GpuTexture, RenderView, Renderer};

/// A view's render textures, by path (`"rt:<name>"`), and its frame count — the
/// clock `update_every` runs on. Per view, so the editor's viewport and Inspector
/// preview, rendering different scenes the same frame, never evict each other's.
#[derive(Default)]
pub struct RenderTextures {
    pub(super) slots: HashMap<String, Slot>,
    pub(super) frame: u64,
}

/// One render texture. The camera draws into `view`'s own target, which is then
/// copied into `front`: consumers only ever sample `front`, so a camera that sees a
/// monitor showing its own picture reads last frame's copy instead of the target
/// it is writing — the hall-of-mirrors, never a read/write hazard.
pub(super) struct Slot {
    pub(super) view: RenderView,
    pub(super) front: Rc<GpuTexture>,
    /// Unique per allocation, so a material bind group built over an old
    /// allocation is never reused for a resized one.
    pub(super) id: u64,
    pub(super) size: (u32, u32),
    /// The view frame this slot last drew on; `None` before its first draw.
    pub(super) drawn_on: Option<u64>,
}

impl Slot {
    /// Whether the camera should redraw this frame: never drawn yet, or at least
    /// `every` frames since the last draw.
    pub(super) fn is_due(&self, frame: u64, every: u32) -> bool {
        is_due(self.drawn_on, frame, every)
    }
}

/// See [`Slot::is_due`]; `every` below 1 means every frame.
pub(super) fn is_due(drawn_on: Option<u64>, frame: u64, every: u32) -> bool {
    drawn_on.is_none_or(|f| frame.saturating_sub(f) >= u64::from(every.max(1)))
}

impl Renderer {
    /// A fresh `width` x `height` slot: its offscreen view and the sampled texture.
    pub(super) fn new_render_texture(&mut self, width: u32, height: u32) -> Slot {
        let (w, h) = (width.max(1), height.max(1));
        let bloom = self.quality.bloom_divisor();
        let view = RenderView::offscreen(&self.device, OFFSCREEN_FORMAT, w, h, bloom);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Render Texture"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Render Texture Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let front = Self::finalize_texture(
            &self.device,
            &self.texture_layout,
            // A rendered picture: a data slot reads its decoded (linear) values,
            // the same the camera shaded, so both views decode (#647).
            (texture, OFFSCREEN_FORMAT),
            sampler,
            Some("Render Texture Bind Group"),
        );
        self.render_texture_ids.1 += 1;
        Slot {
            view,
            front: Rc::new(front),
            id: self.render_texture_ids.1,
            size: (w, h),
            drawn_on: None,
        }
    }
}
