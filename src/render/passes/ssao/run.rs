//! Recording one camera's SSAO: the depth prepass, the occlusion pass and the blur,
//! ahead of the forward pass in the same encoder.

use super::{SsaoFrame, SsaoTargets};
use crate::render::draw::batch::DrawBatch;
use crate::render::gpu::pipelines::surface::SolidPass;
use crate::render::timing::GpuPass;
use crate::render::{RenderView, Renderer};

impl Renderer {
    /// Record `frame`'s SSAO for `solids` into `encoder`, leaving the result in
    /// `view.ssao` for the forward pass's group 3, and count its cost.
    pub(crate) fn record_ssao(
        &mut self,
        view: &mut RenderView,
        encoder: &mut wgpu::CommandEncoder,
        frame: &SsaoFrame,
        solids: &[DrawBatch],
    ) {
        let size = view.size();
        let divisor = frame.plan.tier.divisor;
        if !view
            .ssao
            .as_ref()
            .is_some_and(|t| t.matches(size.width, size.height, divisor))
        {
            view.ssao = Some(SsaoTargets::new(self, size.width, size.height, divisor));
        }
        let targets = view.ssao.as_ref().expect("allocated above");
        self.queue
            .write_buffer(&targets.uniform, 0, bytemuck::bytes_of(&frame.uniform()));
        self.record_prepass(encoder, &targets.depth, solids);
        let timer = &self.gpu_timer;
        let ao = (&targets.ao_group, &targets.raw);
        self.ssao.fullscreen(encoder, false, ao, timer);
        let blur = (&targets.blur_group, &targets.ao);
        self.ssao.fullscreen(encoder, true, blur, timer);

        let (w, h) = targets.raw_size;
        let c = &mut self.frame_counters;
        c.add_draws(
            solids
                .iter()
                .map(|b| (b.num_indices, b.instances.len() as u32)),
        );
        c.ssao_samples += u64::from(w) * u64::from(h) * u64::from(frame.plan.tier.samples);
    }

    /// The forward pass's group 3 for `view`: its AO when the SSAO passes ran this
    /// camera (`ran`), else the white no-AO group.
    pub(crate) fn scene_group3<'a>(
        &'a self,
        view: &'a RenderView,
        ran: bool,
    ) -> &'a wgpu::BindGroup {
        match (ran, &view.ssao) {
            (true, Some(targets)) => &targets.forward_group,
            _ => &self.shadow_bind_group,
        }
    }

    /// The solids' depth alone into `depth`, through the forward vertex stage.
    fn record_prepass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        depth: &wgpu::TextureView,
        solids: &[DrawBatch],
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            multiview_mask: None,
            label: Some("SSAO Depth Prepass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: self.gpu_timer.writes(GpuPass::Ssao),
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &self.global_bind_group, &[]);
        // The prepass reads nothing from group 3; the no-AO group satisfies the layout.
        pass.set_bind_group(3, &self.shadow_bind_group, &[]);
        self.draw_batches(
            &mut pass,
            solids,
            SolidPass::Prepass(&self.prepass_pipeline),
        );
    }
}
