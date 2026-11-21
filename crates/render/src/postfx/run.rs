use super::effects::{self, EffectCx, Gpu, LdrRun, Pass};
use super::{PostFx, PostFxContext, PostParams};
use crate::gpu::commands::Commands;
use crate::gpu::ops::{CountedDevice as _, CountedEncoder as _, CountedQueue as _};
use crate::timing::GpuPass;

impl PostFx {
    pub(crate) fn run(
        &mut self,
        gpu: Gpu,
        commands: &mut Commands,
        ctx: PostFxContext<'_>,
        (cx, params): (&EffectCx, PostParams),
    ) {
        let (device, queue) = gpu;
        queue.counted_write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
        let slots = effects::composite_slots(cx);
        queue.counted_write_buffer(&self.fx_buffer, 0, bytemuck::cast_slice(&slots));
        let ldr: Vec<&LdrRun> = effects::ldr_passes()
            .iter()
            .filter_map(|pass| {
                let Pass::Ldr { prepare, run, .. } = pass else {
                    return None;
                };
                prepare(self, device, cx).then_some(run)
            })
            .collect();

        let encoder = commands.encoder();
        let [io, textures] = self.composite_groups(device, &ctx);
        let followed = !ldr.is_empty();
        let composite_target = if followed { &self.ldr.view } else { ctx.output };
        let composite = (&self.composite_pipeline, ctx.timer.writes(GpuPass::PostFx));
        Self::fullscreen(encoder, composite, &[&io, &textures], composite_target);

        let mut finish = None;
        let mut ended = false;
        let count = ldr.len();
        for (i, run) in ldr.into_iter().enumerate() {
            match run {
                LdrRun::InPlace(run) => run(self, gpu, encoder, &ctx, cx),
                LdrRun::MayEnd(run) => {
                    ended = run(self, gpu, encoder, &ctx, cx, i + 1 == count);
                }
                LdrRun::Finish(pipeline) => finish = Some(*pipeline),
            }
        }
        let last = match finish {
            Some(pipeline) => Some(pipeline(self)),
            None => (followed && !ended).then_some(&self.copy_pipeline),
        };
        if let Some(pipeline) = last {
            let bg = self.io_bind_group(
                device,
                &self.ldr.view,
                self.bloom_view(),
                ctx.depth_view,
                ctx.skybox,
            );
            let last = (pipeline, ctx.timer.writes(GpuPass::PostFx));
            Self::fullscreen(encoder, last, &[&bg], ctx.output);
        }
    }

    pub(crate) fn composite_into(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        ctx: PostFxContext<'_>,
    ) {
        let [io, textures] = self.composite_groups(device, &ctx);
        let composite = (&self.composite_pipeline, ctx.timer.writes(GpuPass::Ui));
        Self::fullscreen(encoder, composite, &[&io, &textures], ctx.output);
    }

    fn composite_groups(&self, device: &wgpu::Device, ctx: &PostFxContext) -> [wgpu::BindGroup; 2] {
        let io = self.io_bind_group(
            device,
            self.scene_color(),
            self.bloom_view(),
            ctx.depth_view,
            ctx.skybox,
        );
        [io, self.effect_textures.bind_group(device, self)]
    }

    pub(crate) fn output_format(&self) -> wgpu::TextureFormat {
        self.output_format
    }

    pub(crate) fn io_bind_group(
        &self,
        device: &wgpu::Device,
        color: &wgpu::TextureView,
        bloom: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        (skybox, skybox_sampler): (&wgpu::TextureView, &wgpu::Sampler),
    ) -> wgpu::BindGroup {
        device.counted_bind_group(&wgpu::BindGroupDescriptor {
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
            ],
        })
    }

    pub(crate) fn fullscreen(
        encoder: &mut wgpu::CommandEncoder,
        timed: Timed<'_>,
        groups: &[&wgpu::BindGroup],
        target: &wgpu::TextureView,
    ) {
        Self::fullscreen_into(encoder, timed, groups, [target]);
    }

    pub(crate) fn fullscreen_into<const N: usize>(
        encoder: &mut wgpu::CommandEncoder,
        (pipeline, timestamp_writes): Timed<'_>,
        groups: &[&wgpu::BindGroup],
        targets: [&wgpu::TextureView; N],
    ) {
        let attachments = targets.map(|view| {
            Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })
        });
        let mut pass = encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
            multiview_mask: None,
            label: Some("PostFX Pass"),
            color_attachments: &attachments,
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

pub(crate) type Timed<'a> = (
    &'a wgpu::RenderPipeline,
    Option<wgpu::RenderPassTimestampWrites<'a>>,
);
