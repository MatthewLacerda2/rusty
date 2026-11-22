//! Phase one draws through the prepass's pipelines, so the depth it keeps equals a full
//! prepass of the final set byte for byte.

pub(crate) mod clusters;
mod counters;
mod cull;
mod drawn;
pub(crate) mod pyramid;
mod stage;
mod tables;
mod upload;

#[cfg(test)]
mod args_gpu_tests;
#[cfg(test)]
mod cluster_floor_gpu_tests;
#[cfg(test)]
pub(crate) mod cluster_gpu_tests;
#[cfg(test)]
mod copy_gpu_tests;
#[cfg(test)]
mod dispatch_gpu_tests;
#[cfg(test)]
mod draws_gpu_tests;
#[cfg(all(test, feature = "dev"))]
mod dust2_gpu_tests;
#[cfg(test)]
mod first_read_gpu_tests;
#[cfg(test)]
mod floor_gpu_tests;
#[cfg(test)]
mod floor_tests;
#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod late_gpu_tests;
#[cfg(test)]
mod moving_gpu_tests;
#[cfg(test)]
mod passes_gpu_tests;
#[cfg(test)]
mod pool_gpu_tests;
#[cfg(test)]
mod projection_gpu_tests;
#[cfg(test)]
mod scale_gpu_tests;
#[cfg(test)]
mod waves_gpu_tests;

use crate::gpu::pipelines::{lazy_compute, LazyCompute};
use crate::gpu::shaders::ShaderRegistry;
use crate::Renderer;

pub(crate) use cull::{CameraCull, Culled, ARGS};

pub(crate) const FEATURE: wgpu::Features = wgpu::Features::INDIRECT_FIRST_INSTANCE;

pub(crate) const MIN_INSTANCES: u32 = 32;

fn culls(supported: bool, (preset, camera): (bool, bool), capturing: bool) -> bool {
    supported && preset && camera && !capturing
}

fn draws_enough((tested, clustered): (usize, impl FnOnce() -> bool), floor: u32) -> bool {
    tested >= floor as usize || clustered()
}

pub(crate) struct HiZ {
    pub(super) cull_layout: wgpu::BindGroupLayout,
    pub(super) select: LazyCompute,
    pub(super) test: LazyCompute,
    pub(super) compact: LazyCompute,
    pub(super) reduce_depth: LazyCompute,
    pub(super) reduce: LazyCompute,
    pub(crate) clusters: clusters::ClusterPipelines,
    pub(crate) min_instances: u32,
}

#[derive(Default)]
pub(crate) struct HiZView {
    pub(crate) pyramid: Option<pyramid::Pyramid>,
    pub(crate) cameras: Vec<CameraCull>,
}

impl HiZ {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let mut registry = ShaderRegistry::new(rusty_shadergen::engine_shader_dir());
        let cull = registry.load(device, "hiz.wgsl", "Hi-Z Cull");
        let levels = registry.load(device, "hiz_pyramid.wgsl", "Hi-Z Pyramid");
        let by_cluster = registry.load(device, "hiz_clusters.wgsl", "Hi-Z Clusters");
        let cull_layout = upload::layout(device);
        let groups: &[&wgpu::BindGroupLayout] = &[&cull_layout];
        let with = |entry| lazy_compute(device, &cull, (entry, entry), Some(groups));
        let (select, test, compact) = (with("cs_select"), with("cs_test"), with("cs_compact"));
        Self {
            select,
            test,
            compact,
            reduce_depth: lazy_compute(device, &levels, ("Hi-Z Depth", "cs_reduce_depth"), None),
            reduce: lazy_compute(device, &levels, ("Hi-Z Reduce", "cs_reduce"), None),
            cull_layout,
            clusters: clusters::ClusterPipelines::new(device, &by_cluster),
            min_instances: MIN_INSTANCES,
        }
    }

    pub(crate) fn lazy_computes(&self) -> [&LazyCompute; 8] {
        let Self {
            select,
            test,
            compact,
            reduce_depth,
            reduce,
            clusters,
            ..
        } = self;
        let c = clusters;
        [
            select,
            test,
            compact,
            reduce_depth,
            reduce,
            &c.test,
            &c.scan,
            &c.copy,
        ]
    }
}

impl Renderer {
    pub(crate) fn hiz_culls(&self, cam: &rusty_scene::Camera) -> bool {
        let supported = self.device.features().contains(FEATURE);
        let on = (
            self.quality.gpu_occlusion_culling,
            cam.use_occlusion_culling,
        );
        culls(supported, on, self.capture.static_only)
    }

    pub(crate) fn hiz_draws_enough(&self, solids: &crate::draw::resources::SolidResources) -> bool {
        let clustered = || self.draws_clustered(&solids.draws);
        draws_enough((solids.tested.len(), clustered), self.hiz.min_instances)
    }

    pub(crate) fn cluster_culls(&self) -> bool {
        self.quality.gpu_cluster_culling
    }
}
