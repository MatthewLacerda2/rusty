//! Recording one camera's SSAO: the depth prepass, the occlusion pass, the blur and
//! the upsample, ahead of the forward pass in the same encoder.

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

/// A depth pass's label, timestamps and depth load ([`Renderer::record_depth`]).
pub(crate) type DepthPass<'a> = (
    &'a str,
    Option<wgpu::RenderPassTimestampWrites<'a>>,
    wgpu::LoadOp<f32>,
);

impl Renderer {
    /// Record `frame`'s SSAO for `solids` into `encoder`, leaving the result in
    /// `view.ssao` for the forward pass's scene group, and count its cost.
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

    /// Record `frame`'s screen-space passes ahead of its forward pass: SSAO's
    /// prepass, occlusion and blur, then SSIL's gather over that prepass's depth
    /// (#1750; it draws its own without SSAO). With neither, a camera the GPU
    /// culls whose forward pass primes still draws the prepass, which is then
    /// only the late set over phase one's depth (#2336), and so does one under
    /// the cull's instance floor, its few batches whole (#2338).
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

    /// Count the prepass's draws of `solids`: a draw per batch it draws, with
    /// every batched instance's triangles (as the forward pass's count), or none
    /// when the GPU culls the camera: the prepass then draws its late set, which
    /// the CPU never sees (`hiz_late` counts it, #2336).
    pub(crate) fn count_prepass(&mut self, solids: Solids) {
        let late = solids.1.is_some();
        let prepass = solids.0.iter().filter(|b| b.key.in_prepass());
        let instances = |b: &crate::draw::batch::DrawBatch| match late {
            true => 0,
            false => b.instances.len() as u32,
        };
        let draws = prepass.map(|b| (b.key.indices.len(), instances(b)));
        self.frame_counters.add_draws(draws);
    }

    /// The forward pass's scene group for `view` under `frame`: SSIL's (with the AO
    /// beside it) when SSIL ran this camera (#1750), else its AO when the SSAO
    /// passes ran this camera, else the white no-AO group.
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

    /// The solids' depth alone into `depth` (the view's, #1578), through the forward
    /// vertex stage. A primed forward pass loads it; any other clears it. SSIL draws
    /// it too, on a camera SSAO skips (#1750).
    ///
    /// A camera the GPU culls (`solids` carries its [`Culled`]) already holds its
    /// phase one's depth there, last frame's visible set: the prepass keeps it and
    /// draws only the late set over it ([`Culled::late`], #2336), which leaves the
    /// depth the whole final set's would. Any other clears it and draws them all.
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
            None => (solids, wgpu::LoadOp::Clear(1.0)),
        };
        let pass = ("Depth Prepass", timed, load);
        self.record_depth(encoder, depth, solids, pass);
    }

    /// The prepass's draw of `solids` into `depth`, loaded or cleared by `load`,
    /// labelled and timed as given: the SSAO prepass, and GPU occlusion culling's
    /// phase one (#1963), which draws last frame's visible instances through it.
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
