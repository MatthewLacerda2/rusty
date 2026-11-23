use super::super::record::{begin, opaque_target, pipeline_layout};
use super::targets::RayTargets;
use crate::gpu::bind_entry as entry;
use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::layout_entries::{sampler_entry, storage_entry, texture_entry, uniform_entry};
use crate::gpu::ops::CountedDevice as _;
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::postfx::HDR_FORMAT;
use crate::timing::{GpuPass, GpuTimer};

const INPUTS: u32 = 0;
const MARCH: u32 = 1;
const SHADE: u32 = 2;
const SOURCE: u32 = 0;

/// `postfx/ssr_rays.wgsl`'s pipelines and its two groups of its own.
pub(crate) struct RayPasses {
    pub(super) shade: wgpu::BindGroupLayout,
    pub(super) merged: wgpu::BindGroupLayout,
    source: wgpu::BindGroupLayout,
    ray: LazyPipeline,
    merge: LazyPipeline,
    resample: LazyPipeline,
}

/// The ray texel's radiance (rgb) and distance (a): `ssr_rays.wgsl`'s `t_rays`.
pub(crate) const RAY_FORMAT: wgpu::TextureFormat = HDR_FORMAT;

fn shade_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let (d2a, frag) = (
        wgpu::TextureViewDimension::D2Array,
        wgpu::ShaderStages::FRAGMENT,
    );
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("SSR Rays Shade Layout"),
        entries: &[
            uniform_entry(0, frag),
            storage_entry(1, frag),
            storage_entry(2, frag),
            texture_entry(3, d2a),
            texture_entry(4, d2a),
            sampler_entry(5),
            texture_entry(6, d2a),
            storage_entry(7, frag),
            sampler_entry(8),
        ],
    })
}

fn resample_pipeline(
    device: &wgpu::Device,
    shader: wgpu::ShaderModule,
) -> (wgpu::BindGroupLayout, LazyPipeline) {
    let source = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("SSR Rays Resample Layout"),
        entries: &[
            texture_entry(10, wgpu::TextureViewDimension::D2),
            sampler_entry(11),
        ],
    });
    let resample_layout = pipeline_layout(device, &[&source]);
    let resample = LazyPipeline::new(device, move |device| {
        let target =
            crate::gpu::textures::mips::view_format(crate::gpu::textures::TextureKind::Colour);
        Fullscreen {
            label: "SSR Rays Resample Pipeline",
            shader: &shader,
            layout: &resample_layout,
            vs: "vs_fullscreen",
            fs: "fs_resample",
        }
        .colour(device, &[Some(opaque_target(target))])
    });
    (source, resample)
}

impl RayPasses {
    /// `inputs` and `march` are SSR's group 0 and its hits' layout.
    pub(crate) fn new(
        device: &wgpu::Device,
        registry: &mut ShaderRegistry,
        (inputs, march): (&wgpu::BindGroupLayout, &wgpu::BindGroupLayout),
    ) -> Self {
        let shader = registry.load(device, "postfx/ssr_rays.wgsl", "SSR Rays Shader");
        let shade = shade_layout(device);
        let merged = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSR Rays Merge Layout"),
            entries: &[texture_entry(9, wgpu::TextureViewDimension::D2)],
        });
        let tree = crate::rays::layout(device);
        let ray_layout = pipeline_layout(device, &[inputs, march, &shade, &tree]);
        let merge_layout = pipeline_layout(device, &[inputs, march, &merged]);
        let shader2 = shader.clone();
        let (source, resample) = resample_pipeline(device, shader.clone());
        let ray = LazyPipeline::new(device, move |device| {
            Fullscreen {
                label: "SSR Rays Pipeline",
                shader: &shader,
                layout: &ray_layout,
                vs: "vs_fullscreen",
                fs: "fs_ray",
            }
            .colour(device, &[Some(opaque_target(RAY_FORMAT))])
        });
        let merge = LazyPipeline::new(device, move |device| {
            let targets = [HDR_FORMAT; 2].map(|f| Some(opaque_target(f)));
            Fullscreen {
                label: "SSR Rays Merge Pipeline",
                shader: &shader2,
                layout: &merge_layout,
                vs: "vs_fullscreen",
                fs: "fs_merge",
            }
            .colour(device, &targets)
        });
        Self {
            shade,
            merged,
            source,
            ray,
            merge,
            resample,
        }
    }

    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 3] {
        [&self.ray, &self.merge, &self.resample]
    }

    /// Draws `source` over all of `target`, a base colour layer's first level.
    pub(super) fn resample(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        (source, sampler): (&wgpu::TextureView, &wgpu::Sampler),
        target: &wgpu::TextureView,
    ) {
        let group = device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSR Rays Resample"),
            layout: &self.source,
            entries: &[
                entry(10, wgpu::BindingResource::TextureView(source)),
                entry(11, wgpu::BindingResource::Sampler(sampler)),
            ],
        });
        let clear = wgpu::LoadOp::Clear(wgpu::Color::WHITE);
        let mut pass = begin(encoder, "SSR Rays Resample", &[(target, clear)], None);
        pass.set_pipeline(self.resample.get());
        pass.set_bind_group(SOURCE, &group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// What a frame's rays bind beside SSR's own group: the shading inputs and the tree.
pub(crate) struct RayGroups<'a> {
    pub inputs: &'a wgpu::BindGroup,
    pub shade: &'a wgpu::BindGroup,
    pub tree: &'a wgpu::BindGroup,
}

/// Traces the march's misses into the rays' target, then merges them with the march's hits
/// (`t`'s) into `tops`, the pyramids' first level.
pub(crate) fn trace_and_merge(
    encoder: &mut wgpu::CommandEncoder,
    passes: &RayPasses,
    groups: RayGroups,
    (t, tops): (&RayTargets, [&wgpu::TextureView; 2]),
    timer: &GpuTimer,
) {
    let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
    {
        let timed = timer.writes(GpuPass::SsrRays);
        let mut pass = begin(encoder, "SSR Rays", &[(&t.rays, clear)], timed);
        pass.set_pipeline(passes.ray.get());
        pass.set_bind_group(INPUTS, groups.inputs, &[]);
        pass.set_bind_group(MARCH, &t.march_group, &[]);
        pass.set_bind_group(SHADE, groups.shade, &[]);
        pass.set_bind_group(crate::rays::GROUP, groups.tree, &[]);
        pass.draw(0..3, 0..1);
    }
    let targets = tops.map(|v| (v, clear));
    let mut pass = begin(encoder, "SSR Rays Merge", &targets, None);
    pass.set_pipeline(passes.merge.get());
    pass.set_bind_group(INPUTS, groups.inputs, &[]);
    pass.set_bind_group(MARCH, &t.march_group, &[]);
    pass.set_bind_group(SHADE, &t.merge_group, &[]);
    pass.draw(0..3, 0..1);
}
