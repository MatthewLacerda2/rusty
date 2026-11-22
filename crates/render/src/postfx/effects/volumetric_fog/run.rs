use crate::gpu::ops::{CountedDevice as _, CountedEncoder as _, CountedQueue as _};
use glam::Vec4;

use super::params::Built;
use super::targets::{History, VolumetricTargets};
use super::{VolumetricParams, VolumetricView};
use crate::draw::camera::CameraCtx;
use crate::gpu::bind_entry as entry;
use crate::timing::GpuPass;
use crate::{RenderView, Renderer};

const INPUTS: u32 = 0;
const GRIDS: u32 = 1;

const SCATTER_GROUP: u32 = 4;
const INTEGRATE_GROUP: u32 = 8;

impl Renderer {
    pub(crate) fn run_volumetric_fog(&mut self, view: &mut RenderView, ctx: &mut CameraCtx) {
        let Some((plan, key)) = ctx.volumetric else {
            return;
        };
        let gpu = (&self.device, &self.queue);
        let baked = &mut self.post_passes.volumetric_fog.baked;
        baked.refresh(ctx.scene, gpu);
        baked.bin(&ctx.clusters, gpu);
        self.global_bind_group_dirty |= self.fog_volume.fit(&self.device, plan.grid);
        let device = &self.device;
        let t = view
            .post_fx
            .targets
            .get_or(|| VolumetricTargets::new(device, plan.grid));
        if !t.matches(plan.grid) {
            *t = VolumetricTargets::new(device, plan.grid);
        }
        let prev = t.advance(key);
        let eye = glam::Vec3::from(ctx.uniform.camera_pos);
        let probes = self.post_passes.volumetric_fog.probes.refresh(
            ctx.scene,
            (eye, plan.depth_extent),
            (&self.queue, &self.staging),
        );
        let params = VolumetricParams::new(&plan, (t.index, probes), prev);
        self.queue
            .counted_write_buffer(&t.uniform, 0, bytemuck::bytes_of(&params));
        let built = Built {
            view_proj: ctx.view_proj,
            view_z: Vec4::from(ctx.uniform.clusters.view_z),
            volumetric: ctx.uniform.volumetric,
        };
        let t = view
            .post_fx
            .targets
            .get::<VolumetricTargets>()
            .expect("fitted above");
        self.dispatch_froxels(ctx.commands.encoder(), t, plan.grid);
        let t = view
            .post_fx
            .targets
            .get_mut::<VolumetricTargets>()
            .expect("fitted above");
        t.history = Some(History { key, built });
        self.frame_counters.volumetric_froxels += plan.froxels();
    }

    fn dispatch_froxels(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        t: &VolumetricTargets,
        [w, h, d]: [u32; 3],
    ) {
        let passes = &self.post_passes.volumetric_fog;
        let (out, history) = (&t.scatter[t.current], &t.scatter[1 - t.current]);
        let scatter = self.scatter_group(t, (out, history));
        let uniforms = self.device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Volumetric Integrate Uniforms"),
            layout: &passes.integrate_layouts[0],
            entries: &[
                entry(0, t.uniform.as_entire_binding()),
                entry(1, self.camera_buffer.uniform.as_entire_binding()),
            ],
        });
        let grids = self.device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Volumetric Integrate Grids"),
            layout: &passes.integrate_layouts[1],
            entries: &[
                entry(0, wgpu::BindingResource::TextureView(out)),
                entry(
                    1,
                    wgpu::BindingResource::TextureView(self.fog_volume.view()),
                ),
            ],
        });
        {
            let timestamp_writes = self.gpu_timer.compute_writes(GpuPass::VolumetricFog);
            let mut pass = encoder.counted_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Volumetric Fog"),
                timestamp_writes,
            });
            pass.set_pipeline(passes.scatter.get());
            pass.set_bind_group(INPUTS, &scatter, &[]);
            let n = SCATTER_GROUP;
            pass.dispatch_workgroups(w.div_ceil(n), h.div_ceil(n), d.div_ceil(n));
            pass.set_pipeline(passes.integrate.get());
            pass.set_bind_group(INPUTS, &uniforms, &[]);
            pass.set_bind_group(GRIDS, &grids, &[]);
            let n = INTEGRATE_GROUP;
            pass.dispatch_workgroups(w.div_ceil(n), h.div_ceil(n), 1);
        }
    }

    fn scatter_group(
        &self,
        t: &VolumetricTargets,
        (out, history): (&wgpu::TextureView, &wgpu::TextureView),
    ) -> wgpu::BindGroup {
        use wgpu::BindingResource::{Sampler, TextureView};
        let passes = &self.post_passes.volumetric_fog;
        let [lights, ranges, indices] = self.clusters.buffers();
        let [baked, baked_ranges, baked_indices] = passes.baked.buffers();
        let shadows = &self.shadow_renderer;
        self.device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Volumetric Scatter"),
            layout: &passes.scatter_layout,
            entries: &[
                entry(0, t.uniform.as_entire_binding()),
                entry(1, self.camera_buffer.uniform.as_entire_binding()),
                entry(2, self.lighting_buffer.as_entire_binding()),
                entry(3, lights.as_entire_binding()),
                entry(4, ranges.as_entire_binding()),
                entry(5, indices.as_entire_binding()),
                entry(6, baked.as_entire_binding()),
                entry(7, baked_ranges.as_entire_binding()),
                entry(8, baked_indices.as_entire_binding()),
                entry(9, self.shadow_uniform_buffer.as_entire_binding()),
                entry(10, TextureView(&shadows.active_view)),
                entry(11, Sampler(&shadows.sampler)),
                entry(12, TextureView(&shadows.atlas.view)),
                entry(13, shadows.atlas.tiles.as_entire_binding()),
                entry(14, TextureView(history)),
                entry(15, Sampler(&passes.sampler)),
                entry(16, TextureView(out)),
                entry(17, TextureView(&passes.probes.view)),
            ],
        })
    }
}

pub(crate) fn for_camera(view: Option<VolumetricView>, is_first: bool) -> Option<VolumetricView> {
    view.filter(|_| is_first)
}
