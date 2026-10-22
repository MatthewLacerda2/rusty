//! Recording one camera's selection outline: the mask, the row dilation and the
//! ring over the HDR target, in one encoder of its own, after the camera's solids
//! and effects. The last two are scissored to the selection's bounds on screen.

use glam::Mat4;

use super::{EdgeUniform, SelectionMask, OUTLINE_COLOR, OUTLINE_PX};
use crate::render::draw::batch::FrameDraws;
use crate::render::draw::resources::SolidResources;
use crate::render::gpu::draw_buffers::DrawBuffers;
use crate::render::gpu::fullscreen::color_pass;
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::timing::GpuPass;
use crate::render::{RenderView, Renderer};

impl Renderer {
    /// Ring the solids `solids` marked for the outline in `view`, seen through
    /// `view_proj` (see the module doc). A no-op when nothing is outlined or the
    /// selection is off screen. `solids` must be this camera's, still uploaded: the
    /// mask binds their uniform slots and instances.
    pub(crate) fn draw_outline(
        &mut self,
        view: &mut RenderView,
        solids: &SolidResources,
        view_proj: Mat4,
    ) {
        let Some(bounds) = solids.outline_bounds else {
            return;
        };
        if solids.draws.outline.is_empty() {
            return;
        }
        let size = view.size();
        let radius = OUTLINE_PX * view.pixel_scale();
        let pad = radius.ceil() as u32 + 1;
        let Some([x, y, w, h]) = bounds.screen_rect(view_proj, (size.width, size.height), pad)
        else {
            return;
        };
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
            radius,
            _pad: [0.0; 3],
        };
        self.queue
            .write_buffer(&mask.uniform, 0, bytemuck::bytes_of(&params));
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Selection Outline Encoder"),
            });
        self.record_mask(&mut encoder, &mask.mask, &solids.draws);
        let rect = [x, y, w, h];
        // Unwritten row distances read as "no mask pixel near".
        let rows = (&mask.rows, wgpu::LoadOp::Clear(wgpu::Color::WHITE));
        let dilate = (&self.outline.dilate, &mask.dilate_group);
        self.scissored(&mut encoder, "Selection Dilate Pass", rows, dilate, rect);
        let hdr = (&view.post_fx.scene_hdr.view, wgpu::LoadOp::Load);
        let edge = (&self.outline.edge, &mask.edge_group);
        self.scissored(&mut encoder, "Selection Edge Pass", hdr, edge, rect);
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
        let timed = ("Selection Mask Pass", &self.gpu_timer, GpuPass::Outline);
        let mut pass = color_pass(encoder, timed, mask, clear);
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

    /// One fullscreen outline pass: `pipeline` with `group` into `target`, timed as
    /// the outline and scissored to `rect` (`[x, y, width, height]`).
    fn scissored(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        label: &str,
        (target, load): (&wgpu::TextureView, wgpu::LoadOp<wgpu::Color>),
        (pipeline, group): (&LazyPipeline, &wgpu::BindGroup),
        [x, y, w, h]: [u32; 4],
    ) {
        let timed = (label, &self.gpu_timer, GpuPass::Outline);
        let mut pass = color_pass(encoder, timed, target, load);
        pass.set_scissor_rect(x, y, w, h);
        pass.set_pipeline(pipeline.get());
        pass.set_bind_group(0, group, &[]);
        pass.draw(0..3, 0..1);
    }
}
