use crate::clusters::ClusterBuffers;
use crate::decals::DecalBuffers;
use crate::gpu::lightmaps::LightmapBinding;
use crate::gpu::ops::CountedDevice as _;

pub(crate) struct GlobalGroup<'a> {
    pub camera: &'a wgpu::Buffer,
    pub lighting: &'a wgpu::Buffer,
    pub skybox: &'a wgpu::TextureView,
    pub cube: &'a wgpu::TextureView,
    pub clusters: &'a ClusterBuffers,
    pub decals: &'a DecalBuffers,
    pub lightmaps: LightmapBinding<'a>,
    pub fog_volume: &'a wgpu::TextureView,
    pub linear_clamp: &'a wgpu::Sampler,
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
            (2, TextureView(self.skybox)),
            (4, TextureView(self.cube)),
            (7, lights.as_entire_binding()),
            (8, ranges.as_entire_binding()),
            (9, indices.as_entire_binding()),
            (10, self.decals.records().as_entire_binding()),
            (11, TextureView(&self.decals.atlas.color_view)),
            (12, TextureView(&self.decals.atlas.data_view)),
            (13, Sampler(&self.decals.atlas.sampler)),
            (14, TextureView(self.lightmaps.pages)),
            (16, TextureView(self.lightmaps.directions)),
            (17, TextureView(self.fog_volume)),
            (18, Sampler(self.linear_clamp)),
            (19, TextureView(self.lightmaps.masks)),
        ];
        let entries: Vec<_> = resources
            .into_iter()
            .map(|(binding, resource)| wgpu::BindGroupEntry { binding, resource })
            .collect();
        device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Global Bind Group"),
            layout,
            entries: &entries,
        })
    }
}
