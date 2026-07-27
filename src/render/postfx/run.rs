//! Per-frame execution of the post-process chain: bright-pass, a 2D gaussian
//! blur, then the composite that writes the corrected image to the target.

use super::{PostFx, PostParams};

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
        } = passes;
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("PostFX Encoder"),
        });

        if bloom_enabled {
            self.run_bloom(device, &mut encoder, ctx.depth_view, ctx.skybox_view);
        }

        // Composite reads scene HDR + bloom_b (final blur result) + depth + skybox.
        // With FXAA on it lands in the LDR target for that pass to sample; with FXAA
        // off it writes the output directly, so the off path is byte-for-byte what
        // the chain produced before this pass existed.
        let composite_bg = self.io_bind_group(
            device,
            &self.scene_hdr.view,
            &self.bloom_b.view,
            ctx.depth_view,
            ctx.skybox_view,
        );
        let composite_target = if fxaa_enabled {
            &self.ldr.view
        } else {
            ctx.output
        };
        Self::fullscreen(
            &mut encoder,
            &self.composite_pipeline,
            &composite_bg,
            composite_target,
        );

        // FXAA: tonemapped LDR in, anti-aliased output out. The aux/depth/skybox
        // bindings are unused by the pass but must satisfy the shared IO layout.
        if fxaa_enabled {
            let fxaa_bg = self.io_bind_group(
                device,
                &self.ldr.view,
                &self.bloom_b.view,
                ctx.depth_view,
                ctx.skybox_view,
            );
            Self::fullscreen(&mut encoder, &self.fxaa_pipeline, &fxaa_bg, ctx.output);
        }

        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Bright-pass (scene HDR -> bloom_a), then a 2D gaussian blur (a -> b),
    /// leaving the blurred bloom in bloom_b for the composite to add.
    fn run_bloom(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
        skybox_view: &wgpu::TextureView,
    ) {
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
        Self::fullscreen(
            encoder,
            &self.bright_pipeline,
            &bright_bg,
            &self.bloom_a.view,
        );

        let blur_bg = self.io_bind_group(
            device,
            &self.bloom_a.view,
            &self.scene_hdr.view,
            depth_view,
            skybox_view,
        );
        Self::fullscreen(encoder, &self.blur_pipeline, &blur_bg, &self.bloom_b.view);
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

    fn fullscreen(
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        bind_group: &wgpu::BindGroup,
        target: &wgpu::TextureView,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("PostFX Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Which optional passes run this frame. Bundled rather than passed as loose bools so
/// `run` stays under the 6-arg threshold — and so two same-typed flags can't be
/// swapped silently at the call site.
#[derive(Clone, Copy, Debug)]
pub struct PostPasses {
    /// Run the bright-pass + blur that feed the composite's bloom add.
    pub bloom: bool,
    /// Run the final anti-aliasing pass (#360).
    pub fxaa: bool,
}

/// The per-frame views the chain reads/writes, bundled to keep `run` under the
/// 6-arg clippy threshold.
pub struct PostFxContext<'a> {
    pub depth_view: &'a wgpu::TextureView,
    pub skybox_view: &'a wgpu::TextureView,
    pub output: &'a wgpu::TextureView,
}
