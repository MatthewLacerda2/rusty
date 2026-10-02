//! The cascades' depth sweeps (#435, #355): re-bake the static layers whose light
//! volume moved, copy the static layers into the active array, then draw the dynamic
//! casters over each cascade.

use glam::Mat4;

use super::cascades::Cascade;
use super::casters::{CasterFrame, Sweep};
use super::ShadowRenderer;
use crate::render::lod::LodSelection;

impl ShadowRenderer {
    /// Record the frame's shadow sweeps: re-bake the static layers whose light volume
    /// moved, copy the static layers into the active array, and draw the dynamic
    /// casters over each cascade — skipping the LOD levels `lod` hides (#472).
    pub(super) fn render_cascades(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &CasterFrame,
        lod: &LodSelection,
    ) {
        let scene = frame.scene;
        let key = |c: &Cascade| Some((scene.id(), c.light_space));
        let stale: Vec<usize> = (0..self.cascades.len())
            .filter(|&i| self.static_cache[i] != key(&self.cascades[i]))
            .collect();
        if !stale.is_empty() {
            // The bake outlives the frame, so it cannot follow the camera's LOD
            // choice: it bakes every group at LOD0 (#472).
            let finest = LodSelection::finest(scene);
            let volumes: Vec<Mat4> = stale
                .iter()
                .map(|&i| self.cascades[i].light_space)
                .collect();
            let batches = self.prepare_casters(frame, &finest, Sweep::Static, &volumes);
            for (&i, batches) in stale.iter().zip(&batches) {
                let view = &self.static_layers[i];
                let mut pass = depth_pass(encoder, "Shadow Static Pass", view, true);
                self.draw_casters(&mut pass, frame, batches, Sweep::Static, i);
            }
            for i in stale {
                self.static_cache[i] = key(&self.cascades[i]);
            }
        }

        self.copy_static_layers(encoder);

        let all: Vec<Mat4> = self.cascades.iter().map(|c| c.light_space).collect();
        let batches = self.prepare_casters(frame, lod, Sweep::Dynamic, &all);
        for (i, batches) in batches.iter().enumerate() {
            if batches.is_empty() {
                continue;
            }
            let view = &self.active_layers[i];
            let mut pass = depth_pass(encoder, "Shadow Dynamic Pass", view, false);
            self.draw_casters(&mut pass, frame, batches, Sweep::Dynamic, i);
        }
    }

    /// Copy the frame's cascade layers from the static bake into the active array.
    fn copy_static_layers(&self, encoder: &mut wgpu::CommandEncoder) {
        let layer = |texture| wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        };
        encoder.copy_texture_to_texture(
            layer(&self.static_texture),
            layer(&self.active_texture),
            wgpu::Extent3d {
                width: Self::CASCADE_SIZE,
                height: Self::CASCADE_SIZE,
                depth_or_array_layers: self.cascades.len().max(1) as u32,
            },
        );
    }
}

/// A depth-only pass over one cascade layer: cleared for a bake, loaded for the
/// dynamic casters drawn over the copied statics.
pub(super) fn depth_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    label: &str,
    view: &'a wgpu::TextureView,
    clear: bool,
) -> wgpu::RenderPass<'a> {
    let load = if clear {
        wgpu::LoadOp::Clear(1.0)
    } else {
        wgpu::LoadOp::Load
    };
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view,
            depth_ops: Some(wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}
