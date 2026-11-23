use crate::gpu::commands::Commands;
use crate::gpu::ops::CountedQueue as _;
use glam::{Mat4, Vec3};

use super::pyramid::{HitPyramids, PyramidPass};
use super::rays::{trace_and_merge, RayGroups, RayPasses, RayTargets};
use super::record::{begin, opaque_target, pipeline_layout};
use super::surface;
pub(crate) use super::targets::SsrTargets;
use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::layout_entries::{sampler_entry, texture_entry, uniform_entry};
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::postfx::{PostFx, HDR_FORMAT};
use crate::timing::{GpuPass, GpuTimer};
use rusty_core::settings::quality::SsrQuality;

const INPUTS: u32 = 0;
const TRACED: u32 = 1;

/// Mirrors `SsrParams` in `postfx/ssr_view.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SsrParams {
    inv_view_proj: [f32; 16],
    view_proj: [f32; 16],
    camera_pos: [f32; 4],
    /// x: step count, y: step length (world units).
    march: [f32; 4],
    surface: [f32; 4],
    /// *Mixed*: the ray's smoothness, reach and divisor, and 1 when rays trace.
    pub(super) rays: [f32; 4],
}

impl SsrParams {
    pub(crate) fn new(view_proj: Mat4, eye: Vec3, m: SsrQuality, min_smoothness: f32) -> Self {
        Self {
            inv_view_proj: view_proj.inverse().to_cols_array(),
            view_proj: view_proj.to_cols_array(),
            camera_pos: eye.extend(0.0).to_array(),
            march: [m.steps as f32, m.step_length, 0.0, 0.0],
            surface: {
                let [min, width] = super::fade(min_smoothness);
                [min, width, 0.0, 0.0]
            },
            rays: [0.0; 4],
        }
    }
}

pub(crate) struct SsrPasses {
    pub(super) layout: wgpu::BindGroupLayout,
    pub(super) traced: wgpu::BindGroupLayout,
    trace: LazyPipeline,
    resolve: LazyPipeline,
    pub(super) pyramid: PyramidPass,
    pub(crate) rays: RayPasses,
}

impl SsrPasses {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "postfx/ssr.wgsl", "SSR Shader");
        let d2 = wgpu::TextureViewDimension::D2;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSR Layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                texture_entry(1, d2),
                sampler_entry(2),
                texture_entry(3, d2),
                texture_entry(4, d2),
                texture_entry(5, d2),
            ],
        });
        let traced = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSR Resolve Layout"),
            entries: &[texture_entry(0, d2), texture_entry(1, d2)],
        });
        let trace_layout = pipeline_layout(device, &[&layout]);
        let resolve_layout = pipeline_layout(device, &[&layout, &traced]);
        let shader2 = shader.clone();
        let trace = LazyPipeline::new(device, move |device| {
            let targets = [HDR_FORMAT; 2].map(|f| Some(opaque_target(f)));
            Fullscreen {
                label: "SSR Trace Pipeline",
                shader: &shader,
                layout: &trace_layout,
                vs: "vs_fullscreen",
                fs: "fs_trace",
            }
            .colour(device, &targets)
        });
        let resolve = LazyPipeline::new(device, move |device| {
            Fullscreen {
                label: "SSR Resolve Pipeline",
                shader: &shader2,
                layout: &resolve_layout,
                vs: "vs_fullscreen",
                fs: "fs_resolve",
            }
            .colour(device, &[Some(opaque_target(HDR_FORMAT))])
        });
        let rays = RayPasses::new(device, registry, (&layout, &traced));
        Self {
            layout,
            traced,
            trace,
            resolve,
            pyramid: PyramidPass::new(device, registry),
            rays,
        }
    }

    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 6] {
        let [ray, merge, resample] = self.rays.lazy_pipelines();
        let pyramid = self.pyramid.lazy_pipeline();
        [&self.trace, &self.resolve, pyramid, ray, merge, resample]
    }
}

impl PostFx {
    pub(crate) fn run_ssr(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        commands: &mut Commands,
        passes: &SsrPasses,
        (params, rays): (Option<SsrParams>, Option<Rays>),
        timer: &GpuTimer,
    ) -> u64 {
        let inputs = self.motion.target_view().is_some() && surface::target(self).is_some();
        let Some(mut params) = params.filter(|_| inputs) else {
            self.targets.remove::<SsrTargets>();
            self.targets.remove::<RayTargets>();
            return 0;
        };
        let rays = rays.map(|r| {
            params.rays = r.params;
            let (size, divisor) = (self.full_size, r.params[2] as u32);
            let t = self
                .targets
                .get_or(|| RayTargets::new(device, (&passes.rays, &passes.traced), size, divisor));
            if !t.fits(size, divisor) {
                *t = RayTargets::new(device, (&passes.rays, &passes.traced), size, divisor);
            }
            r
        });
        if rays.is_none() {
            self.targets.remove::<RayTargets>();
        }
        let ray_targets = self.targets.get::<RayTargets>().cloned();
        let texels = ray_targets.as_ref().map_or(0, RayTargets::texels);
        let size = self.full_size;
        let fitted = self
            .targets
            .get_or(|| SsrTargets::new(device, passes, size));
        if fitted.size != size {
            *fitted = SsrTargets::new(device, passes, size);
        }
        let velocity = self.motion.target_view().expect("checked above").clone();
        let [weight, applied] = surface::views(self).expect("checked above");
        let inputs = [velocity, weight.clone(), applied.clone()];
        let t = self.targets.get_mut::<SsrTargets>().expect("fitted above");
        queue.counted_write_buffer(&t.params, 0, bytemuck::bytes_of(&params));
        #[cfg(test)]
        super::poison::write(t.poisoned, (queue, commands), &self.scene_hdr.texture);
        let group = t.group(device, passes, inputs).clone();

        let encoder = commands.encoder();
        let extent = self.scene_hdr.texture.size();
        let (from, to) = (&self.scene_hdr.texture, &t.colour);
        encoder.copy_texture_to_texture(from.as_image_copy(), to.as_image_copy(), extent);
        let traced = (&t.traced, &t.resolve);
        let rays = rays.as_ref().zip(ray_targets.as_ref());
        trace_and_resolve(
            encoder,
            passes,
            (&group, rays),
            traced,
            &self.scene_hdr.view,
            timer,
        );
        texels
    }
}

/// A frame's rays: the tree, the shading group and `SsrParams::rays`.
pub(crate) struct Rays {
    pub tree: wgpu::BindGroup,
    pub shade: wgpu::BindGroup,
    pub params: [f32; 4],
}

fn trace_and_resolve(
    encoder: &mut wgpu::CommandEncoder,
    passes: &SsrPasses,
    (group, rays): (&wgpu::BindGroup, Option<(&Rays, &RayTargets)>),
    (pyramids, traced): (&HitPyramids, &wgpu::BindGroup),
    scene: &wgpu::TextureView,
    timer: &GpuTimer,
) {
    {
        // Cleared: a pixel that takes no SSR is discarded, so it traced nothing.
        let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
        // Under *Mixed* the march writes its own targets, which the merge folds into the tops.
        let [hits, spread] = match rays {
            Some((_, t)) => [&t.march[0], &t.march[1]],
            None => pyramids.tops(),
        };
        let targets = [(hits, clear), (spread, clear)];
        let timed = timer.writes(GpuPass::Ssr);
        let mut pass = begin(encoder, "SSR Trace", &targets, timed);
        pass.set_pipeline(passes.trace.get());
        pass.set_bind_group(INPUTS, group, &[]);
        pass.draw(0..3, 0..1);
    }
    if let Some((r, t)) = rays {
        let groups = RayGroups {
            inputs: group,
            shade: &r.shade,
            tree: &r.tree,
        };
        trace_and_merge(encoder, &passes.rays, groups, (t, pyramids.tops()), timer);
    }
    pyramids.build(encoder, &passes.pyramid, timer);
    {
        // Loaded: a pixel that takes no SSR is discarded, left as it was.
        let targets = [(scene, wgpu::LoadOp::Load)];
        let timed = timer.writes(GpuPass::SsrResolve);
        let mut pass = begin(encoder, "SSR Resolve", &targets, timed);
        pass.set_pipeline(passes.resolve.get());
        pass.set_bind_group(INPUTS, group, &[]);
        pass.set_bind_group(TRACED, traced, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
impl PostFx {
    pub(crate) fn holds_ssr(&self) -> bool {
        self.targets.get::<SsrTargets>().is_some()
    }

    pub(crate) fn ssr_pyramid(&self) -> Option<((u32, u32), u32, u32)> {
        let t = self.targets.get::<SsrTargets>()?;
        Some((t.size, t.colour.mip_level_count(), t.traced.levels()))
    }
}
