//! Recording one camera's selection outline: the mask, then the edge pass over the
//! HDR target, in one encoder of its own, after the camera's solids and effects.

use super::{EdgeUniform, SelectionMask, OUTLINE_COLOR, OUTLINE_PX};
use crate::render::draw::batch::FrameDraws;
use crate::render::gpu::draw_buffers::DrawBuffers;
use crate::render::{RenderView, Renderer};

impl Renderer {
    /// Ring the solids `draws` marked for the outline in `view` (see the module doc).
    /// A no-op when nothing is outlined. `draws` must be this camera's, still
    /// uploaded: the mask binds their uniform slots and instances.
    pub(crate) fn draw_outline(&mut self, view: &mut RenderView, draws: &FrameDraws) {
        if draws.outline.is_empty() {
            return;
        }
        let size = view.size();
        if !view
            .selection_mask
            .as_ref()
            .is_some_and(|m| m.matches(size.width, size.height))
        {
            let mask = SelectionMask::new(&self.device, &self.outline, size.width, size.height);
            view.selection_mask = Some(mask);
        }
        let mask = view.selection_mask.as_ref().expect("allocated above");
        let params = EdgeUniform {
            color: OUTLINE_COLOR,
            radius: OUTLINE_PX * view.pixel_scale(),
            _pad: [0.0; 3],
        };
        self.queue
            .write_buffer(&mask.uniform, 0, bytemuck::bytes_of(&params));
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Selection Outline Encoder"),
            });
        self.record_mask(&mut encoder, &mask.view, draws);
        let hdr = &view.post_fx.scene_hdr.view;
        let mut pass = colour_pass(&mut encoder, "Selection Edge Pass", hdr, wgpu::LoadOp::Load);
        pass.set_pipeline(self.outline.edge.get());
        pass.set_bind_group(0, &mask.edge_group, &[]);
        pass.draw(0..3, 0..1);
        drop(pass);
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// The outlined draws into the cleared `mask`: each with the forward pass's own
    /// mesh, group-1 slot, material group and instance.
    fn record_mask(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        mask: &wgpu::TextureView,
        draws: &FrameDraws,
    ) {
        let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
        let mut pass = colour_pass(encoder, "Selection Mask Pass", mask, clear);
        pass.set_pipeline(self.outline.mask.get());
        pass.set_bind_group(0, &self.global_bind_group, &[]);
        // Group 3 is in the forward layout; the mask reads nothing from it.
        pass.set_bind_group(3, &self.shadow_bind_group, &[]);
        for draw in &draws.outline {
            let Some(gpu_mesh) = self.gpu_meshes.get(&draw.mesh) else {
                continue;
            };
            pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            let offsets = DrawBuffers::offsets(draw.uniform_slot);
            pass.set_bind_group(1, self.draw_buffers.bind_group(), &offsets);
            pass.set_bind_group(2, self.materials.group(draw.material), &[]);
            pass.draw_indexed(0..draw.num_indices, 0, draw.instance..draw.instance + 1);
        }
    }
}

/// Begin a one-target colour pass into `target`, with no depth attachment.
fn colour_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    label: &str,
    target: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}
