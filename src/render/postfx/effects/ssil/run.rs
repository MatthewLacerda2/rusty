//! Recording a base camera's SSIL (#1750): the gather, blur, accumulate and upsample
//! ahead of its forward pass, in the scene pass's encoder, and the source after it.

use super::pass::{Stage, StageInputs};
use super::targets::{History, SsilTargets};
use super::{Previous, SsilFrame};
use crate::render::draw::batch::DrawBatch;
use crate::render::draw::camera::CameraCtx;
use crate::render::gpu::bind_layouts::create_shadow_bind_group;
use crate::render::passes::transparent::OpaqueViews;
use crate::render::{RenderView, Renderer};

impl Renderer {
    /// Record `frame`'s gather and what follows it into `encoder`, ahead of the
    /// forward pass, and build the group 3 that pass binds with the result
    /// ([`SsilTargets::forward_group`]). `prepassed`: SSAO ran this camera, so its
    /// prepass already holds the solids' depth and its AO goes in that group too;
    /// otherwise SSIL draws the prepass itself.
    pub(crate) fn record_ssil(
        &mut self,
        view: &mut RenderView,
        encoder: &mut wgpu::CommandEncoder,
        frame: &SsilFrame,
        solids: &[DrawBatch],
        prepassed: bool,
    ) {
        let size = view.scene_size();
        let full = (size.width, size.height);
        let divisor = frame.plan.quality.resolution_divisor;
        let device = &self.device;
        let t = view
            .post_fx
            .targets
            .get_or(|| SsilTargets::new(device, full, divisor));
        if !t.matches(full, divisor) {
            *t = SsilTargets::new(device, full, divisor);
        }
        let prev = t.history.filter(|h| h.key == frame.key).map(|h| h.camera);
        t.history = t.history.filter(|_| prev.is_some());
        t.current = 1 - t.current;
        let uniform = frame.uniform(prev);
        self.queue
            .write_buffer(&t.uniform, 0, bytemuck::bytes_of(&uniform));
        if !prepassed {
            self.record_prepass(encoder, &view.depth_view, solids);
            let prepass = solids.iter().filter(|b| b.key.in_prepass());
            let draws = prepass.map(|b| (b.key.indices.len(), b.instances.len() as u32));
            self.frame_counters.add_draws(draws);
        }

        let (passes, timer) = (&self.post_passes.ssil, &self.gpu_timer);
        let depth = &view.depth_view;
        let stage = |encoder: &mut wgpu::CommandEncoder, stage, (input, prev), target| {
            let inputs = StageInputs {
                uniform: &t.uniform,
                depth,
                input,
                prev,
            };
            passes.record((device, encoder), stage, inputs, (target, timer));
        };
        let (source, raw, blurred) = (&t.source.view, &t.raw.view, &t.blurred.view);
        let [result, last] = [&t.results[t.current].view, &t.results[1 - t.current].view];
        stage(encoder, Stage::Gather, (source, source), raw);
        stage(encoder, Stage::Blur, (raw, raw), blurred);
        stage(encoder, Stage::Accumulate, (blurred, last), result);
        if let Some(full) = &t.full {
            stage(encoder, Stage::Upsample, (result, result), &full.view);
        }

        let ao = match (prepassed, &view.ssao) {
            (true, Some(ssao)) => ssao.ao(),
            _ => &self.ssao.no_ao,
        };
        t.forward_group = Some(create_shadow_bind_group(
            device,
            &self.shadow_layout,
            &self.shadow_uniform_buffer,
            &self.shadow_renderer,
            ao,
            // The opaque pass reads no opaque copy: its slot carries the bounce.
            OpaqueViews {
                texture: t.result(),
                ..self.opaque_copy.fallback()
            },
        ));
        let (w, h) = t.raw_size();
        let samples = u64::from(frame.plan.quality.samples);
        self.frame_counters.ssil_samples += u64::from(w) * u64::from(h) * samples;
    }

    /// Keep this frame's realtime light, after `ctx`'s opaque pass and before SSR
    /// and anything that blends over it, as the source next frame's gather reads,
    /// and remember the camera it was seen from. Then clear the camera's SSIL flag:
    /// only its opaque pass reads the bounce and writes the share in its alpha, and
    /// the translucents' alpha blends. Nothing on a camera that ran no SSIL.
    pub(crate) fn keep_ssil_source(&self, view: &mut RenderView, ctx: &mut CameraCtx) {
        let Some(frame) = ctx.ssil else {
            return;
        };
        if let Some(t) = view.post_fx.targets.get_mut::<SsilTargets>() {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("SSIL Source Encoder"),
                });
            let scene = &view.post_fx.scene_hdr.view;
            let inputs = StageInputs {
                uniform: &t.uniform,
                depth: &view.depth_view,
                input: scene,
                prev: scene,
            };
            let gpu = (&self.device, &mut encoder);
            let target = (&t.source.view, &self.gpu_timer);
            (self.post_passes.ssil).record(gpu, Stage::Source, inputs, target);
            self.queue.submit(std::iter::once(encoder.finish()));
            let camera = Previous {
                view_proj: frame.view_proj,
                eye: frame.eye,
            };
            t.history = Some(History {
                key: frame.key,
                camera,
            });
        }
        // After that submit: a write lands before the next one, so the opaque pass
        // read the flag set and everything after it reads it clear.
        ctx.uniform.stack[2] = 0.0;
        let uniform = bytemuck::bytes_of(&ctx.uniform);
        self.queue.write_buffer(&self.camera_buffer, 0, uniform);
    }
}
