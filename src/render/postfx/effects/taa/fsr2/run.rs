//! One FSR2 frame ([`PostFx::run_fsr2`]): the uniform, each pass's bind group and
//! the five dispatches in one compute pass, timed as `fsr2`.

use super::params::{Frame, Fsr2Params};
use super::{Fsr2Passes, Fsr2Targets};
use crate::render::gpu::bind_entry as entry;
use crate::render::postfx::effects::taa::{reactive, TaaHistory};
use crate::render::postfx::PostFx;
use crate::render::timing::{GpuPass, GpuTimer};

/// The compute passes' workgroup edge (FSR2's 8×8, `@workgroup_size(8, 8)`).
const GROUP: u32 = 8;

/// What FSR2 reads of the view's camera and volume this frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fsr2Inputs {
    /// The volume's fixed EV, and whether its exposure is the meter's (Automatic).
    pub exposure: (f32, bool),
    /// The base camera's view-space offset per NDC unit at depth 1.
    pub lens: [f32; 2],
}

fn view(t: &wgpu::TextureView) -> wgpu::BindingResource<'_> {
    wgpu::BindingResource::TextureView(t)
}

impl Fsr2Passes {
    fn group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        entries: &[wgpu::BindGroupEntry],
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("FSR2 Bind Group"),
            layout,
            entries,
        })
    }
}

impl PostFx {
    /// Upscale this frame through FSR2 into [`Fsr2Targets::output`], in TAA's place:
    /// `draw_stack` held the TAA history (jitter, reactive mask) and FSR2's targets
    /// together, through `taa::begin`, so a view holds both or neither.
    pub(crate) fn run_fsr2(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        passes: &Fsr2Passes,
        timer: &GpuTimer,
        inputs: Fsr2Inputs,
    ) {
        let drawn = self.take_reactive_drawn();
        let (Some(h), Some(f), Some(velocity)) = (
            self.targets.get::<TaaHistory>(),
            self.targets.get::<Fsr2Targets>(),
            self.motion.target_view(),
        ) else {
            return;
        };
        let params = Fsr2Params::new(&Frame {
            sizes: [f.render, f.display],
            jitter: h.pixels,
            index: f.frame,
            phases: h.phases,
            reactive: drawn,
            exposure: inputs.exposure,
            lens: inputs.lens,
        });
        queue.write_buffer(&f.params, 0, bytemuck::bytes_of(&params));
        let (now, then) = (f.current, 1 - f.current);
        let color = self.focused_color();
        let l = &passes.layouts;
        let shared = passes.group(
            device,
            &l.shared,
            &[
                entry(0, f.params.as_entire_binding()),
                entry(1, self.exposure.state().as_entire_binding()),
            ],
        );
        let sampler = wgpu::BindingResource::Sampler(&passes.sampler);
        let luma = passes.group(
            device,
            &l.luma,
            &[
                entry(0, view(color)),
                entry(1, sampler.clone()),
                entry(2, view(&f.luma_mip.view)),
            ],
        );
        let reconstruct = passes.group(
            device,
            &l.reconstruct,
            &[
                entry(0, view(velocity)),
                entry(1, view(color)),
                entry(2, f.prev_depth.as_entire_binding()),
                entry(3, view(&f.dilated_depth.view)),
                entry(4, view(&f.dilated_motion[now].view)),
                entry(5, view(&f.lock_luma.view)),
            ],
        );
        let depth_clip = passes.group(
            device,
            &l.depth_clip,
            &[
                entry(0, f.prev_depth.as_entire_binding()),
                entry(1, view(&f.dilated_motion[now].view)),
                entry(2, view(&f.dilated_depth.view)),
                entry(3, view(&h.reactive.view)),
                entry(4, view(&f.dilated_motion[then].view)),
                entry(5, view(velocity)),
                entry(6, view(color)),
                entry(7, sampler.clone()),
                entry(8, view(&f.prepared.view)),
                entry(9, view(&f.reactive_masks.view)),
            ],
        );
        let lock = passes.group(
            device,
            &l.lock,
            &[
                entry(0, view(&f.lock_luma.view)),
                entry(1, f.new_locks.as_entire_binding()),
                entry(2, f.prev_depth.as_entire_binding()),
            ],
        );
        let accumulate = passes.group(
            device,
            &l.accumulate,
            &[
                entry(0, view(&f.prepared.view)),
                entry(1, view(&f.reactive_masks.view)),
                entry(2, view(&f.dilated_motion[now].view)),
                entry(3, view(&f.history[then].view)),
                entry(4, view(&f.lock_status[then].view)),
                entry(5, view(&f.luma_history[then].view)),
                entry(6, view(&f.luma_mip.view)),
                entry(7, sampler),
                entry(8, f.new_locks.as_entire_binding()),
                entry(9, view(&f.history[now].view)),
                entry(10, view(&f.lock_status[now].view)),
                entry(11, view(&f.luma_history[now].view)),
                entry(12, view(&f.output.view)),
            ],
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("FSR2 Encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("FSR2"),
                timestamp_writes: timer.compute_writes(GpuPass::Fsr2),
            });
            pass.set_bind_group(0, &shared, &[]);
            let (luma_w, luma_h) = (f.luma_mip.texture.width(), f.luma_mip.texture.height());
            let render = (f.render.0.div_ceil(GROUP), f.render.1.div_ceil(GROUP));
            let display = (f.display.0.div_ceil(GROUP), f.display.1.div_ceil(GROUP));
            let dispatches = [
                (passes.luma.get(), &luma, (luma_w, luma_h)),
                (passes.reconstruct.get(), &reconstruct, render),
                (passes.depth_clip.get(), &depth_clip, render),
                (passes.lock.get(), &lock, render),
                (passes.accumulate.get(), &accumulate, display),
            ];
            for (pipeline, group, (x, y)) in dispatches {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(1, group, &[]);
                pass.dispatch_workgroups(x, y, 1);
            }
        }
        if drawn {
            reactive::clear(&mut encoder, &h.reactive);
        }
        queue.submit(std::iter::once(encoder.finish()));
        let f = self.targets.get_mut::<Fsr2Targets>().expect("held above");
        (f.current, f.frame, f.ran) = (then, f.frame.saturating_add(1), true);
        let h = self.targets.get_mut::<TaaHistory>().expect("held above");
        h.ran = true;
    }
}
