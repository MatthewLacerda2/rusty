use crate::gpu::ops::CountedEncoder as _;
use std::sync::atomic::Ordering;

use crate::postfx::{PfxTarget, PostFx};

use super::TaaHistory;

pub(crate) const REACTIVE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

pub(crate) fn target_state() -> wgpu::ColorTargetState {
    let max = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Max,
    };
    wgpu::ColorTargetState {
        format: REACTIVE_FORMAT,
        blend: Some(wgpu::BlendState {
            color: max,
            alpha: max,
        }),
        write_mask: wgpu::ColorWrites::RED,
    }
}

pub(super) fn create(device: &wgpu::Device, (w, h): (u32, u32)) -> PfxTarget {
    PostFx::target(device, w, h, "TAA Reactive", REACTIVE_FORMAT)
}

pub(crate) fn attachment(view: &wgpu::TextureView) -> wgpu::RenderPassColorAttachment<'_> {
    wgpu::RenderPassColorAttachment {
        depth_slice: None,
        view,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
        },
    }
}

pub(crate) fn hdr_attachments(
    view: &crate::RenderView,
) -> Vec<Option<wgpu::RenderPassColorAttachment<'_>>> {
    let colour = attachment(&view.post_fx.scene_hdr.view);
    let reactive = view.post_fx.taa_reactive().map(attachment);
    if let Some(h) = view.post_fx.targets.get::<TaaHistory>() {
        h.reactive_drawn.store(true, Ordering::Relaxed);
    }
    std::iter::once(Some(colour))
        .chain(reactive.map(Some))
        .collect()
}

pub(super) fn clear(
    encoder: &mut wgpu::CommandEncoder,
    target: &PfxTarget,
    timed: Option<wgpu::RenderPassTimestampWrites>,
) {
    encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
        multiview_mask: None,
        label: Some("TAA Reactive Clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            depth_slice: None,
            view: &target.view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: timed,
        occlusion_query_set: None,
    });
}

impl PostFx {
    pub(crate) fn taa_reactive(&self) -> Option<&wgpu::TextureView> {
        let h = self.targets.get::<TaaHistory>()?;
        Some(&h.reactive.view)
    }

    pub(super) fn take_reactive_drawn(&self) -> bool {
        let h = self.targets.get::<TaaHistory>();
        h.is_some_and(|h| h.reactive_drawn.swap(false, Ordering::Relaxed))
    }
}
