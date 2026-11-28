use super::{SsaoFrame, SsaoTargets, Stage};
use crate::draw::pass::ScenePassFrame;
use crate::draw::record::Solids;
use crate::gpu::groups;
use crate::gpu::ops::{CountedEncoder as _, CountedQueue as _};
use crate::gpu::pipelines::surface::SolidPass;
use crate::passes::hiz::Culled;
use crate::postfx::effects::ssil;
use crate::timing::GpuPass;
use crate::{RenderView, Renderer};

pub(crate) type DepthPass<'a> = (
    &'a str,
    Option<wgpu::RenderPassTimestampWrites<'a>>,
    wgpu::LoadOp<f32>,
);

impl Renderer {
    pub(crate) fn record_ssao(
        &mut self,
        view: &mut RenderView,
        encoder: &mut wgpu::CommandEncoder,
        frame: &SsaoFrame,
        solids: Solids,
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
            .counted_write_buffer(&targets.uniform, 0, bytemuck::bytes_of(&frame.uniform()));
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
        self.count_prepass(solids);
        let c = &mut self.frame_counters;
        c.ssao_samples += u64::from(w) * u64::from(h) * u64::from(frame.plan.quality.samples);
    }

    pub(crate) fn record_screen_space(
        &mut self,
        view: &mut RenderView,
        encoder: &mut wgpu::CommandEncoder,
        frame: &ScenePassFrame,
        solids: Solids,
    ) {
        if !frame.screen_space && self.primes(view, frame) {
            self.record_prepass(encoder, &view.depth_view, solids);
            self.count_prepass(solids);
        }
        if let Some(ssao) = &frame.ssao {
            self.record_ssao(view, encoder, ssao, solids);
        }
        if let Some(ssil) = &frame.ssil {
            self.record_ssil(view, encoder, ssil, solids, frame.ssao.is_some());
        }
    }

    pub(crate) fn count_prepass(&mut self, solids: Solids) {
        let prepass = solids.0.iter().filter(|b| b.key.in_prepass());
        let draws = prepass.map(|b| (b.key.indices.len(), b.instances.len() as u32));
        let read = solids.1.map(|culled| culled.triangles.map(|t| t.late));
        self.frame_counters.add_read_draws(draws, read.flatten());
    }

    pub(crate) fn scene_group<'a>(
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

    pub(crate) fn record_prepass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        depth: &wgpu::TextureView,
        solids: Solids,
    ) {
        let timed = self.gpu_timer.writes(GpuPass::DepthPrepass);
        let late = solids.1.and_then(Culled::late);
        let (solids, load) = match &late {
            Some(late) => ((solids.0, Some(late)), wgpu::LoadOp::Load),
            None => (solids, crate::gpu::depth::CLEAR),
        };
        let pass = ("Depth Prepass", timed, load);
        self.record_depth(encoder, depth, solids, pass);
    }

    pub(crate) fn record_depth(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        depth: &wgpu::TextureView,
        solids: Solids,
        (label, timestamp_writes, load): DepthPass,
    ) {
        let mut pass = encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
            multiview_mask: None,
            label: Some(label),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes,
            occlusion_query_set: None,
        });
        pass.set_bind_group(groups::GLOBAL, &self.global_bind_group, &[]);
        // The prepass reads nothing from the scene group; the no-AO one satisfies the layout.
        pass.set_bind_group(groups::SCENE, &self.shadow_bind_group, &[]);
        self.draw_solids(
            &mut pass,
            solids,
            SolidPass::Prepass(&self.prepass_pipeline),
        );
    }
}
