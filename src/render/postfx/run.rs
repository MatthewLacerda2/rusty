//! Per-frame execution of the post-process chain after the effects' scene passes:
//! the composite (assembled from the effects' functions around the grading LUT
//! `grade` baked), the LDR passes (the authored effects, #397, then FXAA — or a
//! plain copy) that write the finished image to the target.

use super::effects::{self, EffectCx, Gpu, LdrRun, Pass};
use super::{PfxTarget, PostFx, PostFxContext, PostParams};
use crate::render::timing::GpuPass;

impl PostFx {
    /// Run the chain after the effects' scene passes: the composite, then the LDR
    /// passes that run for `cx` (the authored effects in place, FXAA finishing), or
    /// a plain copy when only in-place ones ran. `params` is the frame: `ctx.output`
    /// the final swapchain/screenshot view.
    pub(crate) fn run(
        &mut self,
        gpu: Gpu,
        ctx: PostFxContext<'_>,
        cx: &EffectCx,
        params: PostParams,
    ) {
        let (device, queue) = gpu;
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
        let slots = effects::composite_slots(cx);
        queue.write_buffer(&self.fx_buffer, 0, bytemuck::cast_slice(&slots));
        let ldr: Vec<&LdrRun> = effects::ldr_passes()
            .iter()
            .filter_map(|pass| {
                let Pass::Ldr { prepare, run, .. } = pass else {
                    return None;
                };
                prepare(self, device, cx).then_some(run)
            })
            .collect();

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("PostFX Encoder"),
        });
        // When an LDR pass follows the composite it lands in the LDR target for that
        // pass to sample; otherwise it writes the output directly, so the plain path
        // is byte-for-byte what the chain produced before those existed.
        let composite_bg = self.io_bind_group(
            device,
            self.scene_color(),
            &self.bloom.b.view,
            ctx.depth_view,
            ctx.skybox,
        );
        let followed = !ldr.is_empty();
        let composite_target = if followed { &self.ldr.view } else { ctx.output };
        let composite = (&self.composite_pipeline, ctx.timer.writes(GpuPass::PostFx));
        Self::fullscreen(&mut encoder, composite, &[&composite_bg], composite_target);

        let mut finish = None;
        for run in ldr {
            match run {
                LdrRun::InPlace(run) => run(self, gpu, &mut encoder, &ctx, cx),
                LdrRun::Finish(pipeline) => finish = Some(*pipeline),
            }
        }
        // The finishing pass, or the copy that lands the in-place passes' result.
        // The aux/depth/skybox bindings are unused by either but must satisfy the
        // shared IO layout.
        let last = match finish {
            Some(pipeline) => Some(pipeline(self)),
            None => followed.then_some(&self.copy_pipeline),
        };
        if let Some(pipeline) = last {
            let bg = self.io_bind_group(
                device,
                &self.ldr.view,
                &self.bloom.b.view,
                ctx.depth_view,
                ctx.skybox,
            );
            let last = (pipeline, ctx.timer.writes(GpuPass::PostFx));
            Self::fullscreen(&mut encoder, last, &[&bg], ctx.output);
        }

        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Composite this frame's scene once more into `ctx.output` — graded,
    /// tonemapped and bloomed, without FXAA or the authored effects — as the source
    /// of the UI's backdrop blur (#426), which runs after the chain at its own,
    /// downsampled size (every read is by uv). Uses the params and bloom `run` left.
    pub(crate) fn composite_into(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        ctx: PostFxContext<'_>,
    ) {
        let bg = self.io_bind_group(
            device,
            self.scene_color(),
            &self.bloom.b.view,
            ctx.depth_view,
            ctx.skybox,
        );
        // The UI's backdrop source, so its time is the UI's.
        let composite = (&self.composite_pipeline, ctx.timer.writes(GpuPass::Ui));
        Self::fullscreen(encoder, composite, &[&bg], ctx.output);
    }

    /// The scene the composite and bloom read: motion-blurred (#1436), else focused.
    /// The lens flare's pass adds into it (#1443), after bloom's bright pass read it.
    pub(crate) fn scene_color(&self) -> &wgpu::TextureView {
        scene_of(self.targets.get(), self.targets.get(), &self.scene_hdr)
    }

    /// The scene motion blur reads: this frame's depth of field when it ran (#1445),
    /// else the HDR scene itself. Depth of field runs first, Unity's order.
    pub(crate) fn focused_color(&self) -> &wgpu::TextureView {
        scene_of(None, self.targets.get(), &self.scene_hdr)
    }

    /// The exposure meter and [`PostFx::scene_color`] at once, so the meter reads
    /// the image after depth of field and motion blur (#1444) without a borrow clash.
    pub(crate) fn exposure_and_scene(
        &mut self,
    ) -> (&mut effects::exposure::ExposureMeter, &wgpu::TextureView) {
        let scene = scene_of(self.targets.get(), self.targets.get(), &self.scene_hdr);
        (&mut self.exposure, scene)
    }

    /// The format the chain writes its output in (what [`PostFx::composite_into`]
    /// must target).
    pub(crate) fn output_format(&self) -> wgpu::TextureFormat {
        self.output_format
    }

    /// Build a bind group: params + color(1) + sampler(2) + bloom(3) + depth(4) +
    /// skybox(5) + its sampler(6) + the grading LUT(7) + the adapted exposure(8) + the
    /// effect slots(9) + bloom's lens dirt(10). `color` is the texture the pass reads, `bloom` an aux texture.
    pub(crate) fn io_bind_group(
        &self,
        device: &wgpu::Device,
        color: &wgpu::TextureView,
        bloom: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        (skybox, skybox_sampler): (&wgpu::TextureView, &wgpu::Sampler),
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PostFX IO Bind Group"),
            layout: &self.io_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(color),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(bloom),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(skybox),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(skybox_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(&self.grade.view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: self.exposure.state().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: self.fx_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(self.bloom.dirt_view()),
                },
            ],
        })
    }

    /// One fullscreen triangle into `target`, `groups` bound from group 0 up, with
    /// the pass's timestamp writes (#835).
    pub(crate) fn fullscreen(
        encoder: &mut wgpu::CommandEncoder,
        (pipeline, timestamp_writes): Timed<'_>,
        groups: &[&wgpu::BindGroup],
        target: &wgpu::TextureView,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            multiview_mask: None,
            label: Some("PostFX Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes,
            occlusion_query_set: None,
        });
        pass.set_pipeline(pipeline);
        for (i, group) in groups.iter().enumerate() {
            pass.set_bind_group(i as u32, *group, &[]);
        }
        pass.draw(0..3, 0..1);
    }
}

/// A pass's pipeline and its timestamp writes.
pub(crate) type Timed<'a> = (
    &'a wgpu::RenderPipeline,
    Option<wgpu::RenderPassTimestampWrites<'a>>,
);

/// The motion-blurred image when `blur` ran, else the focused one when `dof` did.
fn scene_of<'a>(
    blur: Option<&'a effects::motion_blur::BlurTargets>,
    dof: Option<&'a effects::depth_of_field::DofTargets>,
    scene_hdr: &'a PfxTarget,
) -> &'a wgpu::TextureView {
    let focused = dof.map_or(&scene_hdr.view, |d| &d.focused.view);
    blur.map_or(focused, |b| &b.blurred.view)
}
