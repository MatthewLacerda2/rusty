use crate::gpu::groups;
use crate::gpu::ops::CountedQueue as _;
use glam::Mat4;

use super::{EdgeUniform, SelectionMask, OUTLINE_COLOR, OUTLINE_PX};
use crate::draw::batch::FrameDraws;
use crate::draw::resources::SolidResources;
use crate::gpu::draw_buffers::DrawBuffers;
use crate::gpu::fullscreen::color_pass;
use crate::gpu::pipelines::LazyPipeline;
use crate::timing::GpuPass;
use crate::{RenderView, Renderer};

impl Renderer {
    pub(crate) fn draw_outline(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &mut RenderView,
        solids: &SolidResources,
        view_proj: Mat4,
        output: &wgpu::TextureView,
    ) {
        let Some(bounds) = solids.outline_bounds else {
            return;
        };
        if solids.draws.outline.is_empty() {
            return;
        }
        let size = view.scene_size();
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
            .counted_write_buffer(&mask.uniform, 0, bytemuck::bytes_of(&params));
        self.record_mask(encoder, &mask.mask, &solids.draws);
        let rect = [x, y, w, h];
        // White is `edge.wgsl`'s `NONE`: a row distance the scissor skips reads as no mask near.
        let rows = (&mask.rows, wgpu::LoadOp::Clear(wgpu::Color::WHITE));
        let dilate = (&self.outline.dilate, &mask.dilate_group);
        self.scissored(encoder, "Selection Dilate Pass", rows, dilate, rect);
        let finished = (output, wgpu::LoadOp::Load);
        let edge = (&self.outline.edge, &mask.edge_group);
        self.scissored(encoder, "Selection Edge Pass", finished, edge, rect);
    }

    fn record_mask(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        mask: &wgpu::TextureView,
        draws: &FrameDraws,
    ) {
        let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
        let timed = ("Selection Mask Pass", &self.gpu_timer, GpuPass::Outline);
        let mut pass = color_pass(encoder, timed, mask, clear);
        pass.set_bind_group(groups::GLOBAL, &self.global_bind_group, &[]);
        // The mask reads nothing from the scene group; the no-AO one satisfies the layout.
        pass.set_bind_group(groups::SCENE, &self.shadow_bind_group, &[]);
        let mut bound = None;
        for draw in &draws.outline {
            let Some(gpu_mesh) = self.gpu_mesh_of(draw.mesh) else {
                continue;
            };
            if bound != Some(draw.facing) {
                pass.set_pipeline(self.outline.mask.get(draw.facing));
                bound = Some(draw.facing);
            }
            pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            let offsets = DrawBuffers::offsets(draw.uniform_slot);
            pass.set_bind_group(groups::DRAW, self.draw_buffers.bind_group(), &offsets);
            pass.set_bind_group(groups::MATERIAL, self.materials.group(draw.material), &[]);
            pass.draw_indexed(draw.indices.clone(), 0, draw.instance..draw.instance + 1);
        }
    }

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
        pass.set_bind_group(crate::gpu::ONLY_GROUP, group, &[]);
        pass.draw(0..3, 0..1);
    }
}
