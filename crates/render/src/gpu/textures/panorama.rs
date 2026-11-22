use std::rc::Rc;

use crate::{GpuTexture, Renderer};
use rusty_asset::sidecar::FilterMode;
use rusty_asset::MapRef;

/// Clamped in V: a repeating V would blend the top row into the bottom one at each pole.
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
    pub(crate) fn load_panorama(&mut self, path: &str) -> Rc<GpuTexture> {
        let image = self.load_texture(&MapRef::from(path));
        Rc::new(Self::finalize_texture(
            &self.device,
            &self.texture_layout,
            (
                image.texture.clone(),
                image.texture.format().remove_srgb_suffix(),
            ),
            (panorama_sampler(&self.device), FilterMode::Trilinear),
            Some(path),
        ))
    }
}
