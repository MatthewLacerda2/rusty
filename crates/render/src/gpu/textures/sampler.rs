use rusty_asset::sidecar::{FilterMode, TextureSettings, WrapMode};

pub(crate) fn sampler(device: &wgpu::Device, settings: &TextureSettings) -> wgpu::Sampler {
    device.create_sampler(&descriptor(settings))
}

pub(crate) fn ui_sampler(device: &wgpu::Device, filter: FilterMode) -> wgpu::Sampler {
    device.create_sampler(&ui_descriptor(filter))
}

fn ui_descriptor(filter: FilterMode) -> wgpu::SamplerDescriptor<'static> {
    let settings = TextureSettings {
        wrap_mode: WrapMode::Clamp,
        filter_mode: filter,
        ..Default::default()
    };
    wgpu::SamplerDescriptor {
        label: Some("UI Image Sampler"),
        ..descriptor(&settings)
    }
}

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

    #[test]
    fn a_ui_image_takes_the_filter_but_always_clamps() {
        let point = ui_descriptor(FilterMode::Point);
        assert_eq!(point.address_mode_u, wgpu::AddressMode::ClampToEdge);
        assert_eq!(point.address_mode_v, wgpu::AddressMode::ClampToEdge);
        assert_eq!(point.mag_filter, wgpu::FilterMode::Nearest);
        assert_eq!(point.min_filter, wgpu::FilterMode::Nearest);

        let trilinear = ui_descriptor(FilterMode::Trilinear);
        assert_eq!(trilinear.address_mode_u, wgpu::AddressMode::ClampToEdge);
        assert_eq!(trilinear.mag_filter, wgpu::FilterMode::Linear);
        assert_eq!(trilinear.mipmap_filter, wgpu::MipmapFilterMode::Linear);
        assert_eq!(trilinear.lod_max_clamp, 32.0, "a UI image reads its mips");
    }
}
