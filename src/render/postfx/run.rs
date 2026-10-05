//! Per-frame execution of the post-process chain: bright-pass, a 2D gaussian
//! blur, the composite (tonemap + grade), the authored effects (#397), then FXAA —
//! or a plain copy — that writes the finished image to the target.

use super::custom::{CustomChain, Effect};
use super::{PfxTarget, PostFx, PostFxContext, PostParams, PostPasses};
use crate::render::timing::GpuPass;

impl PostFx {
    /// Run the full chain. `depth_view` is the scene depth (for SSR + motion blur),
    /// `skybox_view` the cubemap fallback, `output` the final swapchain/screenshot
    /// view. `passes` says which optional passes run this frame.
    pub fn run(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        ctx: PostFxContext<'_>,
        params: PostParams,
        passes: PostPasses,
    ) {
        let PostPasses {
            bloom: bloom_enabled,
            fxaa: fxaa_enabled,
            custom,
            post_params,
        } = passes;
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
        self.custom.prepare(device, &custom);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("PostFX Encoder"),
        });

        if bloom_enabled {
            self.run_bloom(device, &mut encoder, &ctx);
        }

        // Composite reads scene HDR + bloom_b (final blur result) + depth + skybox.
        // When anything follows it (FXAA, authored effects) it lands in the LDR target
        // for that pass to sample; otherwise it writes the output directly, so the
        // plain path is byte-for-byte what the chain produced before those existed.
        let effects = self.custom.loaded(&custom);
        CustomChain::write_params(queue, &effects, &post_params);
        let composite_bg = self.io_bind_group(
            device,
            &self.scene_hdr.view,
            &self.bloom_b.view,
            ctx.depth_view,
            ctx.skybox_view,
        );
        let followed = fxaa_enabled || !effects.is_empty();
        let composite_target = if followed { &self.ldr.view } else { ctx.output };
        let composite = (&self.composite_pipeline, ctx.timer.writes(GpuPass::PostFx));
        Self::fullscreen(&mut encoder, composite, &[&composite_bg], composite_target);
        let finished = self.run_custom(device, &mut encoder, &ctx, &effects);

        // FXAA: tonemapped LDR in, anti-aliased output out. The aux/depth/skybox
        // bindings are unused by the pass but must satisfy the shared IO layout.
        // Without FXAA, authored effects still need their result copied out.
        let last = match (fxaa_enabled, effects.is_empty()) {
            (true, _) => Some(&self.fxaa_pipeline),
            (false, false) => Some(&self.copy_pipeline),
            (false, true) => None,
        };
        if let Some(pipeline) = last {
            let bg = self.io_bind_group(
                device,
                &finished.view,
                &self.bloom_b.view,
                ctx.depth_view,
                ctx.skybox_view,
            );
            let last = (pipeline, ctx.timer.writes(GpuPass::PostFx));
            Self::fullscreen(&mut encoder, last, &[&bg], ctx.output);
        }

        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Run the authored effects (#397) over the composite in `ldr`, ping-ponging with
    /// the second LDR target, and keep the result as next frame's history. Returns the
    /// target holding the result (`ldr` itself when there are no effects).
    fn run_custom<'s>(
        &'s self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        ctx: &PostFxContext<'_>,
        effects: &[&Effect],
    ) -> &'s PfxTarget {
        let (mut src, mut dst) = (&self.ldr, &self.custom.ldr_b);
        if effects.is_empty() {
            return src;
        }
        for effect in effects {
            let bg = self.io_bind_group(
                device,
                &src.view,
                &self.custom.history.view,
                ctx.depth_view,
                ctx.skybox_view,
            );
            let pass = (&effect.pipeline, ctx.timer.writes(GpuPass::PostFx));
            Self::fullscreen(encoder, pass, &[&bg, &effect.params], &dst.view);
            (src, dst) = (dst, src);
        }
        encoder.copy_texture_to_texture(
            src.texture.as_image_copy(),
            self.custom.history.texture.as_image_copy(),
            src.texture.size(),
        );
        src
    }

    /// Bright-pass (scene HDR -> bloom_a), then a 2D gaussian blur (a -> b),
    /// leaving the blurred bloom in bloom_b for the composite to add.
    fn run_bloom(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        ctx: &PostFxContext<'_>,
    ) {
        let (depth_view, skybox_view) = (ctx.depth_view, ctx.skybox_view);
        // The aux "bloom" binding (slot 3) is unused by the bright/blur passes, so
        // it points at scene_hdr (never a render target here) to avoid a sampled-
        // texture / color-target usage conflict.
        let bright_bg = self.io_bind_group(
            device,
            &self.scene_hdr.view,
            &self.scene_hdr.view,
            depth_view,
            skybox_view,
        );
        let bright = (&self.bright_pipeline, ctx.timer.writes(GpuPass::PostFx));
        Self::fullscreen(encoder, bright, &[&bright_bg], &self.bloom_a.view);

        let blur_bg = self.io_bind_group(
            device,
            &self.bloom_a.view,
            &self.scene_hdr.view,
            depth_view,
            skybox_view,
        );
        let blur = (&self.blur_pipeline, ctx.timer.writes(GpuPass::PostFx));
        Self::fullscreen(encoder, blur, &[&blur_bg], &self.bloom_b.view);
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
            &self.scene_hdr.view,
            &self.bloom_b.view,
            ctx.depth_view,
            ctx.skybox_view,
        );
        // The UI's backdrop source, so its time is the UI's.
        let composite = (&self.composite_pipeline, ctx.timer.writes(GpuPass::Ui));
        Self::fullscreen(encoder, composite, &[&bg], ctx.output);
    }

    /// The format the chain writes its output in (what [`PostFx::composite_into`]
    /// must target).
    pub(crate) fn output_format(&self) -> wgpu::TextureFormat {
        self.output_format
    }

    /// Build a bind group: params + color(1) + sampler(2) + bloom(3) + depth(4) +
    /// skybox(5). `color` is the texture the pass reads, `bloom` an aux texture.
    fn io_bind_group(
        &self,
        device: &wgpu::Device,
        color: &wgpu::TextureView,
        bloom: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        skybox: &wgpu::TextureView,
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
            ],
        })
    }

    /// One fullscreen triangle into `target`, `groups` bound from group 0 up, with
    /// the pass's timestamp writes (#835).
    fn fullscreen(
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
type Timed<'a> = (
    &'a wgpu::RenderPipeline,
    Option<wgpu::RenderPassTimestampWrites<'a>>,
);
