//! The forward renderer's group 0 (camera, lighting, skybox, reflection cube and the
//! light clusters), built in one place: at setup and whenever a resource it names
//! is swapped (a new skybox or probe cube, a grown cluster buffer).

use crate::render::clusters::ClusterBuffers;

/// What group 0 binds; see `bind_layouts::create_camera_lighting_layout`.
pub(crate) struct GlobalGroup<'a> {
    pub camera: &'a wgpu::Buffer,
    pub lighting: &'a wgpu::Buffer,
    pub skybox: (&'a wgpu::TextureView, &'a wgpu::Sampler),
    pub cube: (&'a wgpu::TextureView, &'a wgpu::Sampler),
    pub clusters: &'a ClusterBuffers,
}

impl GlobalGroup<'_> {
    pub(crate) fn create(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        use wgpu::BindingResource::{Sampler, TextureView};
        let [lights, ranges, indices] = self.clusters.buffers();
        let resources = [
            (0, self.camera.as_entire_binding()),
            (1, self.lighting.as_entire_binding()),
            (2, TextureView(self.skybox.0)),
            (3, Sampler(self.skybox.1)),
            (4, TextureView(self.cube.0)),
            (5, Sampler(self.cube.1)),
            (7, lights.as_entire_binding()),
            (8, ranges.as_entire_binding()),
            (9, indices.as_entire_binding()),
        ];
        let entries: Vec<_> = resources
            .into_iter()
            .map(|(binding, resource)| wgpu::BindGroupEntry { binding, resource })
            .collect();
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Global Bind Group"),
            layout,
            entries: &entries,
        })
    }
}
