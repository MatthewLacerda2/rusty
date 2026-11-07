//! TAA's reactive mask (#1845), FSR2's *reactive mask*: a scene-size target the
//! translucent passes (transparent surfaces, sprite particles, ribbons) write into,
//! blended by max, saying how much of each pixel they laid down this frame
//! (`engine/shaders/reactive.wgsl`). The resolve raises the current frame's weight
//! toward 1 by it (`frame_weight` in `postfx/taa.wgsl`), so a moving translucent,
//! which writes no motion of its own and is reprojected with what is behind it,
//! stays crisp instead of averaging with that surface's history.
//!
//! **Who writes what** (a view runs TAA, so it holds this target):
//! - the transparent pass: every translucent, through `fs_transparent`, its alpha,
//!   or what it added or multiplied (Additive, Multiply), or its material's `Off` /
//!   `Full` (`TaaReactive`);
//! - the sprite particles and ribbons: their `fs_*_reactive` twins, alike, an
//!   emitter's `ParticleRender::reactive` replacing the automatic value;
//! - nothing else: the opaque passes, the depth postpass (a translucent that writes
//!   its motion is reprojected as a solid instead), decals (folded into the surface
//!   under them, moving with it) and world canvases write none.
//!
//! A view without TAA (no history: a bake capture, a view with TAA off) holds no
//! target and draws its translucents through the pipelines it always did, byte for
//! byte. The target is cleared after each resolve that read it, so a frame with no
//! translucent reads zero everywhere and resolves exactly as before; such a frame
//! (no pass bound the mask, `TaaHistory::reactive_drawn`) skips the read and the
//! clear altogether.

use std::sync::atomic::Ordering;

use crate::render::postfx::{PfxTarget, PostFx};

use super::TaaHistory;

/// What the target holds: one packed value, the index in its stack of the topmost
/// camera that drew a translucent there plus that camera's reactivity (below 1),
/// 0 where none drew (`engine/shaders/reactive.wgsl`). Blended by max, a higher
/// camera's write replaces a lower one's, so the resolve ignores a lower camera's
/// translucent under a higher camera's surface (a world window behind a
/// viewmodel's gun). Half-float keeps a reactivity step under 1/256 up to the
/// eighth camera.
pub(crate) const REACTIVE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

/// The colour target a reactive pipeline writes at location 1: blended by max, so
/// overlapping translucents keep the topmost camera's most reactive.
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

/// The target at the scene's size `(w, h)`.
pub(super) fn create(device: &wgpu::Device, (w, h): (u32, u32)) -> PfxTarget {
    PostFx::target(device, w, h, "TAA Reactive", REACTIVE_FORMAT)
}

/// The colour attachment a translucent pass loads and adds to.
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

/// The colour attachments of a translucent pass into `view`: its HDR target, then
/// this mask where the view runs TAA. Every translucent pass (the transparent
/// surfaces, `passes::blended`'s particles and ribbons) begins with these, so its
/// target count matches its pipelines' (`target_state` at location 1).
pub(crate) fn hdr_attachments(
    view: &crate::render::RenderView,
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

/// Clear the target after the resolve read it, so next frame starts from zero.
pub(super) fn clear(encoder: &mut wgpu::CommandEncoder, target: &PfxTarget) {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
        timestamp_writes: None,
        occlusion_query_set: None,
    });
}

impl PostFx {
    /// The reactive target this frame's translucents write, while the view runs
    /// TAA; `None` draws them as a view without TAA does.
    pub(crate) fn taa_reactive(&self) -> Option<&wgpu::TextureView> {
        let h = self.targets.get::<TaaHistory>()?;
        Some(&h.reactive.view)
    }

    /// Whether a translucent pass bound the mask since the last resolve, and reset
    /// it: the resolve reads and clears the mask only then.
    pub(super) fn take_reactive_drawn(&self) -> bool {
        let h = self.targets.get::<TaaHistory>();
        h.is_some_and(|h| h.reactive_drawn.swap(false, Ordering::Relaxed))
    }
}
