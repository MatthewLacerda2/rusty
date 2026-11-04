//! Recording one camera's SSAO: the depth prepass, the occlusion pass, the blur and
//! the upsample, ahead of the forward pass in the same encoder.

use super::{SsaoFrame, SsaoTargets, Stage};
use crate::render::draw::batch::DrawBatch;
use crate::render::draw::pass::ScenePassFrame;
use crate::render::gpu::pipelines::surface::SolidPass;
use crate::render::postfx::effects::ssil;
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
        let size = view.scene_size();
        let divisor = frame.plan.quality.resolution_divisor;
        if !view
            .ssao
            .as_ref()
            .is_some_and(|t| t.matches(size.width, size.height, divisor))
        {
            let full = (size.width, size.height);
            view.ssao = Some(SsaoTargets::new(self, &view.depth_view, full, divisor));
        }
        let targets = view.ssao.as_ref().expect("allocated above");
        self.queue
            .write_buffer(&targets.uniform, 0, bytemuck::bytes_of(&frame.uniform()));
        self.record_prepass(encoder, &view.depth_view, solids);
        let (ssao, timer) = (&self.ssao, &self.gpu_timer);
        let ao = (&targets.ao_group, &targets.raw);
        ssao.fullscreen(encoder, Stage::Occlusion, ao, timer);
        let blur = (&targets.blur_group, targets.blur_target());
        ssao.fullscreen(encoder, Stage::Blur, blur, timer);
        if let Some(group) = &targets.upsample_group {
            ssao.fullscreen(encoder, Stage::Upsample, (group, &targets.ao), timer);
        }

        let (w, h) = targets.raw_size;
        let c = &mut self.frame_counters;
        c.add_draws(
            solids
                .iter()
                .filter(|b| b.key.in_prepass())
                .map(|b| (b.key.indices.len(), b.instances.len() as u32)),
        );
        c.ssao_samples += u64::from(w) * u64::from(h) * u64::from(frame.plan.quality.samples);
    }

    /// Record `frame`'s screen-space passes ahead of its forward pass: SSAO's
    /// prepass, occlusion and blur, then SSIL's gather over that prepass's depth
    /// (#1750; it draws its own without SSAO).
    pub(crate) fn record_screen_space(
        &mut self,
        view: &mut RenderView,
        encoder: &mut wgpu::CommandEncoder,
        frame: &ScenePassFrame,
        solids: &[DrawBatch],
    ) {
        if let Some(ssao) = &frame.ssao {
            self.record_ssao(view, encoder, ssao, solids);
        }
        if let Some(ssil) = &frame.ssil {
            self.record_ssil(view, encoder, ssil, solids, frame.ssao.is_some());
        }
    }

    /// The forward pass's group 3 for `view` under `frame`: SSIL's (with the AO
    /// beside it) when SSIL ran this camera (#1750), else its AO when the SSAO
    /// passes ran this camera, else the white no-AO group.
    pub(crate) fn scene_group3<'a>(
        &'a self,
        view: &'a RenderView,
        frame: &ScenePassFrame,
    ) -> &'a wgpu::BindGroup {
        let ssil = ssil::forward_group(&view.post_fx).filter(|_| frame.ssil.is_some());
        if let Some(group) = ssil {
            return group;
        }
        match (frame.ssao.is_some(), &view.ssao) {
            (true, Some(targets)) => &targets.forward_group,
            _ => &self.shadow_bind_group,
        }
    }

    /// The solids' depth alone into `depth` (the view's, #1578), through the forward
    /// vertex stage. A primed forward pass loads it; any other clears it. SSIL draws
    /// it too, on a camera SSAO skips (#1750).
    pub(crate) fn record_prepass(
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
            timestamp_writes: self.gpu_timer.writes(GpuPass::SsaoPrepass),
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
