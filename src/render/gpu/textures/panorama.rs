//! The skybox panorama's own sampler (#1037). An equirect image wraps in U (φ = ±π
//! is one seam) but not in V: v = θ/π runs zenith (0) to nadir (1), so a repeating
//! V blends the top row into the bottom one at each pole under linear filtering.

use std::rc::Rc;

use crate::asset::MapRef;
use crate::render::{GpuTexture, Renderer};

/// Repeat in U, clamp in V, filtered like every other image texture.
fn panorama_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Skybox Panorama Sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        anisotropy_clamp: 16,
        ..Default::default()
    })
}

impl Renderer {
    /// Load `path` as a skybox panorama: the cached image texture (shared with any
    /// material using the same file, which keeps its repeating sampler) re-bound
    /// with [`panorama_sampler`]. A failed load yields the default texture, likewise.
    pub(crate) fn load_panorama(&mut self, path: &str) -> Rc<GpuTexture> {
        let image = self.load_texture(&MapRef::from(path));
        Rc::new(Self::finalize_texture(
            &self.device,
            &self.texture_layout,
            (image.texture.clone(), super::DATA_FORMAT),
            panorama_sampler(&self.device),
            Some(path),
        ))
    }
}
