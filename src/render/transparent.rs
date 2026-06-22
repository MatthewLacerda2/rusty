//! src/render/transparent.rs — the alpha-blended transparent solids pass (#242).
//!
//! Drawn after opaque solids + decals, before particles/post-FX. The draw list is the
//! Transparent-material entities for one camera, already sorted back-to-front by the
//! `precreate_solid_resources` split, so `ALPHA_BLENDING` composites correctly (nearer
//! glass over farther glass over the opaque world).
//!
//! It reuses the same per-entity pool bind groups the opaque pass uses (group 1
//! entity, group 2 material) and the same shader; only the pipeline differs
//! (`transparent_pipeline`: alpha blend, depth test on / write off). It LOADs the
//! existing HDR colour + depth — never clears — so it layers onto the opaque image and
//! is occluded by opaque depth without writing depth itself (overlapping translucent
//! surfaces all blend rather than z-cull each other).

use super::draw_resources::TransparentResource;
use super::Renderer;

impl Renderer {
    /// Draw the camera's translucent solids into the HDR target. `items` is already
    /// sorted back-to-front (farthest first). No-op when there is nothing translucent,
    /// so opaque-only scenes pay nothing (#242).
    pub(super) fn draw_transparent(&self, items: &[TransparentResource]) {
        if items.is_empty() {
            return;
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Transparent Encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Transparent Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.post_fx.scene_hdr.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    // Load the opaque depth so glass is occluded by solids in front,
                    // but the transparent pipeline writes no depth (store keeps it as-is
                    // for any later pass that reads it).
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.record_transparent(&mut pass, items);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Record the back-to-front transparent draws into `pass`: the transparent
    /// pipeline + global/shadow bind groups, then each entity's pooled entity/material
    /// bind groups and geometry (the same resources the opaque pass binds).
    fn record_transparent<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        items: &'a [TransparentResource],
    ) {
        let pool = self.entity_pool.as_ref().expect("entity pool present");
        pass.set_pipeline(&self.transparent_pipeline);
        pass.set_bind_group(0, &self.global_bind_group, &[]);
        pass.set_bind_group(3, &self.shadow_bind_group, &[]);
        for ((id, mesh_id, num_indices), _depth) in items {
            let (Some(gpu_mesh), Some(entity_bg), Some(material_bg)) = (
                self.gpu_meshes.get(mesh_id),
                pool.entity_bind_group(*id),
                pool.material_bind_group(*id),
            ) else {
                continue;
            };
            pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.set_bind_group(1, entity_bg, &[]);
            pass.set_bind_group(2, material_bg, &[]);
            pass.draw_indexed(0..*num_indices, 0, 0..1);
        }
    }
}
