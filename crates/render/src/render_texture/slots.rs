use std::collections::HashMap;
use std::rc::Rc;

use crate::setup::headless::OFFSCREEN_FORMAT;
use crate::{GpuTexture, RenderView, Renderer};
use rusty_asset::sidecar::FilterMode;

#[derive(Default)]
pub struct RenderTextures {
    pub(super) slots: HashMap<String, Slot>,
    pub(super) frame: u64,
}

pub(super) struct Slot {
    pub(super) view: RenderView,
    pub(super) front: Rc<GpuTexture>,
    pub(super) id: u64,
    pub(super) size: (u32, u32),
    pub(super) drawn_on: Option<u64>,
}

impl Slot {
    pub(super) fn is_due(&self, frame: u64, every: u32) -> bool {
        is_due(self.drawn_on, frame, every)
    }
}

pub(super) fn frames_since(drawn_on: Option<u64>, frame: u64) -> u32 {
    let gap = drawn_on.map_or(1, |f| frame.saturating_sub(f).max(1));
    u32::try_from(gap).unwrap_or(u32::MAX)
}

pub(super) fn is_due(drawn_on: Option<u64>, frame: u64, every: u32) -> bool {
    drawn_on.is_none_or(|f| frame.saturating_sub(f) >= u64::from(every.max(1)))
}

impl Renderer {
    pub(super) fn new_render_texture(&mut self, width: u32, height: u32) -> Slot {
        let (w, h) = (width.max(1), height.max(1));
        let view = RenderView::offscreen(&self.device, OFFSCREEN_FORMAT, w, h);
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
            (texture, OFFSCREEN_FORMAT),
            (sampler, FilterMode::Bilinear),
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
