use super::probes::AirProbes;
use super::volume::FROXEL_FORMAT;
use crate::clusters::BakedLamps;
use crate::gpu::layout_entries::{
    depth_entry, sampler_entry, storage_entry, texture_entry, uniform_entry,
};
use crate::gpu::pipelines::{lazy_compute, LazyCompute};
use crate::gpu::shaders::ShaderRegistry;

pub(crate) struct VolumetricFogPasses {
    pub(super) scatter_layout: wgpu::BindGroupLayout,
    pub(super) integrate_layouts: [wgpu::BindGroupLayout; 2],
    pub(super) scatter: LazyCompute,
    pub(super) integrate: LazyCompute,
    pub(super) sampler: wgpu::Sampler,
    pub(crate) baked: BakedLamps,
    pub(crate) probes: AirProbes,
}

fn compute(entry: wgpu::BindGroupLayoutEntry) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        visibility: wgpu::ShaderStages::COMPUTE,
        ..entry
    }
}

fn grid_out(binding: u32) -> wgpu::BindGroupLayoutEntry {
    compute(wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: FROXEL_FORMAT,
            view_dimension: wgpu::TextureViewDimension::D3,
        },
        count: None,
    })
}

impl VolumetricFogPasses {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        use wgpu::TextureViewDimension::D3;
        let module = registry.load(device, "volumetric_fog.wgsl", "Volumetric Fog");
        let uniform = |b| uniform_entry(b, wgpu::ShaderStages::COMPUTE);
        let scatter_layout = scatter_layout(device);
        let integrate_layouts = [
            layout(
                device,
                "Volumetric Integrate Uniforms",
                &[uniform(0), uniform(1)],
            ),
            layout(
                device,
                "Volumetric Integrate Grids",
                &[compute(texture_entry(0, D3)), grid_out(1)],
            ),
        ];
        let scatter = lazy_compute(
            device,
            &module,
            ("Volumetric Scatter", "cs_scatter"),
            Some(&[&scatter_layout]),
        );
        let [uniforms, grids] = &integrate_layouts;
        let names = ("Volumetric Integrate", "cs_integrate");
        let integrate = lazy_compute(device, &module, names, Some(&[uniforms, grids]));
        Self {
            scatter,
            integrate,
            scatter_layout,
            integrate_layouts,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Volumetric History Sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            baked: BakedLamps::new(device),
            probes: AirProbes::new(device),
        }
    }

    pub(crate) fn lazy_computes(&self) -> [&LazyCompute; 2] {
        [&self.scatter, &self.integrate]
    }
}

fn layout(
    device: &wgpu::Device,
    label: &str,
    entries: &[wgpu::BindGroupLayoutEntry],
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries,
    })
}

fn scatter_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    use wgpu::TextureViewDimension::{D2Array, D2, D3};
    let uniform = |b| uniform_entry(b, wgpu::ShaderStages::COMPUTE);
    let storage = |b| storage_entry(b, wgpu::ShaderStages::COMPUTE);
    let comparison = wgpu::BindGroupLayoutEntry {
        binding: 11,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
        count: None,
    };
    layout(
        device,
        "Volumetric Scatter",
        &[
            uniform(0),
            uniform(1),
            uniform(2),
            storage(3),
            storage(4),
            storage(5),
            storage(6),
            storage(7),
            storage(8),
            uniform(9),
            compute(depth_entry(10, D2Array)),
            comparison,
            compute(depth_entry(12, D2)),
            storage(13),
            compute(texture_entry(14, D3)),
            compute(sampler_entry(15)),
            grid_out(16),
            compute(texture_entry(17, D3)),
        ],
    )
}
