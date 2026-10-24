//! A texture's sampler, built from its import settings (#1323): Unity keeps wrap
//! and filter on the texture, so every upload carries its own sampler and a
//! material binds each map's beside it (`render::draw::materials`).

use crate::asset::sidecar::{FilterMode, TextureSettings, WrapMode};

/// The sampler `settings` describe. Trilinear (the default: repeating, all three
/// filters linear) is anisotropic too; wgpu allows anisotropy only when every
/// filter is linear, so Point and Bilinear sample without it.
pub(crate) fn sampler(device: &wgpu::Device, settings: &TextureSettings) -> wgpu::Sampler {
    device.create_sampler(&descriptor(settings))
}

/// What [`sampler`] builds, apart from the device so a test can read it.
pub(super) fn descriptor(settings: &TextureSettings) -> wgpu::SamplerDescriptor<'static> {
    use wgpu::{FilterMode as F, MipmapFilterMode as M};
    let address = match settings.wrap_mode {
        WrapMode::Repeat => wgpu::AddressMode::Repeat,
        WrapMode::Clamp => wgpu::AddressMode::ClampToEdge,
        WrapMode::Mirror => wgpu::AddressMode::MirrorRepeat,
    };
    let (texel, mip, anisotropy_clamp) = match settings.filter_mode {
        FilterMode::Point => (F::Nearest, M::Nearest, 1),
        FilterMode::Bilinear => (F::Linear, M::Nearest, 1),
        FilterMode::Trilinear => (F::Linear, M::Linear, 16),
    };
    wgpu::SamplerDescriptor {
        label: Some("Image Sampler"),
        address_mode_u: address,
        address_mode_v: address,
        address_mode_w: address,
        mag_filter: texel,
        min_filter: texel,
        mipmap_filter: mip,
        anisotropy_clamp,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_setting_reaches_the_sampler() {
        let default = descriptor(&TextureSettings::default());
        assert_eq!(default.address_mode_u, wgpu::AddressMode::Repeat);
        assert_eq!(default.mipmap_filter, wgpu::MipmapFilterMode::Linear);
        assert_eq!(default.anisotropy_clamp, 16);

        let point = descriptor(&TextureSettings {
            wrap_mode: WrapMode::Clamp,
            filter_mode: FilterMode::Point,
            ..Default::default()
        });
        assert_eq!(point.address_mode_v, wgpu::AddressMode::ClampToEdge);
        assert_eq!(point.mag_filter, wgpu::FilterMode::Nearest);
        assert_eq!(point.anisotropy_clamp, 1, "anisotropy needs linear filters");

        let bilinear = descriptor(&TextureSettings {
            wrap_mode: WrapMode::Mirror,
            filter_mode: FilterMode::Bilinear,
            ..Default::default()
        });
        assert_eq!(bilinear.address_mode_u, wgpu::AddressMode::MirrorRepeat);
        assert_eq!(bilinear.min_filter, wgpu::FilterMode::Linear);
        assert_eq!(bilinear.mipmap_filter, wgpu::MipmapFilterMode::Nearest);
    }
}
