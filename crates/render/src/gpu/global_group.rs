//! The forward renderer's group 0 (camera, lighting, skybox, reflection cube, the
//! light clusters, the decals and the lightmap pages), built in one place: at setup
//! and whenever a resource it names is swapped (a new skybox or probe cube array, a grown
//! cluster or decal buffer, a grown decal atlas, a rebaked lightmap set (its
//! shadowmask pages too, #1695), a
//! volumetric fog volume of another grid).

use crate::clusters::ClusterBuffers;
use crate::decals::DecalBuffers;
use crate::gpu::lightmaps::LightmapBinding;
use crate::gpu::ops::CountedDevice as _;

/// What group 0 binds; see `bind_layouts::create_camera_lighting_layout`.
pub(crate) struct GlobalGroup<'a> {
    pub camera: &'a wgpu::Buffer,
    pub lighting: &'a wgpu::Buffer,
    /// The sky's reflection cube (#1553).
    pub skybox: &'a wgpu::TextureView,
    /// The resident reflection probes' cube array (#1206).
    pub cube: &'a wgpu::TextureView,
    pub clusters: &'a ClusterBuffers,
    pub decals: &'a DecalBuffers,
    /// The baked lightmap pages (#438), their direction (#810) and shadowmask
    /// (#1695) pages.
    pub lightmaps: LightmapBinding<'a>,
    /// The integrated volumetric fog (#1703).
    pub fog_volume: &'a wgpu::TextureView,
    /// The one sampler the sky, the probe cubes, the lightmaps, the shadowmask pages
    /// (binding 19) and the fog volume read through ([`super::linear_clamp`]); the
    /// decal atlas keeps its own, so give none of them a sampler of their own.
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
