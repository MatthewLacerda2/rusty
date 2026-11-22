use std::rc::Rc;

use image::GenericImageView;

use super::{compressed, deep, sampler, upload, MipSettings, TextureKind, DATA_FORMAT};
use crate::{GpuTexture, Renderer};
use rusty_asset::sidecar::{TextureSettings, TextureType};
use rusty_asset::texture_import::{self, Texels};
use rusty_asset::MapRef;

impl Renderer {
    pub fn load_texture(&mut self, map: &MapRef) -> Rc<GpuTexture> {
        self.load_texture_as(map, TextureKind::Colour)
    }

    pub(crate) fn load_texture_as(&mut self, map: &MapRef, kind: TextureKind) -> Rc<GpuTexture> {
        let path_str = map.as_str();
        if let Some(tex) = self.gpu_textures.get(path_str) {
            self.texture_kinds.requested(path_str, kind, "material");
            if deep::serves(tex, kind) {
                return Rc::clone(tex);
            }
            return self.fallback(map, kind);
        }
        if self.texture_freshness.is_miss(path_str) {
            return Rc::clone(&self.default_texture);
        }
        // Never on disk: the default, uncached, until the frame that draws it registers it.
        if crate::render_texture::is_render_texture(path_str) {
            return Rc::clone(&self.default_texture);
        }

        let Some(tex) = self.upload_map(map, kind) else {
            self.texture_freshness.miss(path_str);
            return Rc::clone(&self.default_texture);
        };
        self.texture_kinds.uploaded(path_str, kind);
        self.uploads.add(compressed::gpu_bytes(&tex.texture));
        self.gpu_textures.insert(map.clone(), Rc::clone(&tex));
        tex
    }

    pub(crate) fn texture_for(&self, map: &MapRef, kind: TextureKind) -> Option<Rc<GpuTexture>> {
        let tex = self.gpu_textures.get(map)?;
        match deep::serves(tex, kind) {
            true => Some(Rc::clone(tex)),
            false => self.texture_fallbacks.get(map).cloned(),
        }
    }

    fn fallback(&mut self, map: &MapRef, kind: TextureKind) -> Rc<GpuTexture> {
        if let Some(tex) = self.texture_fallbacks.get(map) {
            return Rc::clone(tex);
        }
        let Some(tex) = self.upload_map(map, kind) else {
            return Rc::clone(&self.default_texture);
        };
        self.uploads.add(compressed::gpu_bytes(&tex.texture));
        self.texture_fallbacks.insert(map.clone(), Rc::clone(&tex));
        tex
    }

    pub(crate) fn upload_map(&self, map: &MapRef, kind: TextureKind) -> Option<Rc<GpuTexture>> {
        let decoded = decode(map, kind, self.device.features())?;
        Some(self.upload_decoded(map, kind, decoded))
    }

    pub(crate) fn install_decoded(
        &mut self,
        map: &MapRef,
        kind: TextureKind,
        decoded: Option<Decoded>,
    ) {
        if self.texture_for(map, kind).is_some() {
            return;
        }
        let first = !self.gpu_textures.contains_key(map);
        let Some(tex) = decoded.map(|d| self.upload_decoded(map, kind, d)) else {
            if first {
                self.texture_freshness.miss(map.as_str());
            }
            return;
        };
        self.uploads.add(compressed::gpu_bytes(&tex.texture));
        match first {
            true => {
                self.texture_kinds.uploaded(map.as_str(), kind);
                self.gpu_textures.insert(map.clone(), tex);
            }
            false => {
                self.texture_kinds.requested(map.as_str(), kind, "material");
                self.texture_fallbacks.insert(map.clone(), tex);
            }
        }
    }

    pub(crate) fn texture_settled(&self, map: &MapRef, kind: TextureKind) -> bool {
        self.texture_for(map, kind).is_some()
            || self.texture_freshness.is_miss(map.as_str())
            || crate::render_texture::is_render_texture(map.as_str())
    }

    fn upload_decoded(&self, map: &MapRef, kind: TextureKind, decoded: Decoded) -> Rc<GpuTexture> {
        let Decoded { import, loaded } = decoded;
        let srgb = !texture_import::is_linear(&import);
        let mut texture = match &loaded.texels {
            Texels::Blocks(blocks) => {
                let gpu = (&self.device, (&self.queue, &self.staging));
                let texture = compressed::upload(gpu, map.as_str(), blocks, srgb);
                Self::finalize_texture(
                    &self.device,
                    &self.texture_layout,
                    texture,
                    (sampler(&self.device, &import), import.filter_mode),
                    Some(map.as_str()),
                )
            }
            Texels::Image(img) => self.upload_image_texture(map.as_str(), img, &import, kind),
        };
        texture.source_size = loaded.source_size;
        texture.single_channel = import.texture_type == TextureType::SingleChannel;
        texture.deep_normal = loaded.deep_normal;
        texture.normal_variance = match &loaded.texels {
            Texels::Blocks(b) if b.format == texture_import::BlockFormat::Bc5 => Some(b.variance),
            _ => None,
        };
        Rc::new(texture)
    }

    fn upload_image_texture(
        &self,
        path_str: &str,
        img: &image::DynamicImage,
        import: &TextureSettings,
        kind: TextureKind,
    ) -> GpuTexture {
        let settings = MipSettings::for_map(import, kind);
        let (width, height) = img.dimensions();
        let levels = settings.levels(width, height);
        let gpu = (&self.device, (&self.queue, &self.staging));
        let srgb = !texture_import::is_linear(import);
        let (texture, data_format) = match deep::wanted(import, img, kind) {
            true => {
                let converted;
                let image = match img.as_rgba16() {
                    Some(image) => image,
                    None => {
                        converted = img.to_rgba16();
                        &converted
                    }
                };
                let texture = deep::upload(gpu, path_str, image, levels);
                (texture, deep::DEEP_FORMAT)
            }
            false => (
                upload(gpu, path_str, &img.to_rgba8(), (levels, srgb)),
                DATA_FORMAT,
            ),
        };
        self.mip_generator
            .generate(&self.device, &self.queue, &texture, settings.kind);
        Self::finalize_texture(
            &self.device,
            &self.texture_layout,
            (texture, data_format),
            (sampler(&self.device, import), import.filter_mode),
            Some(path_str),
        )
    }
}

pub(crate) struct Decoded {
    import: TextureSettings,
    loaded: texture_import::Loaded,
}

pub(crate) fn decode(map: &MapRef, kind: TextureKind, features: wgpu::Features) -> Option<Decoded> {
    let file = map.file()?;
    let import = rusty_asset::sidecar::texture(&file);
    let compress = compressed::wanted_for(map.as_str(), features, &import, kind);
    let loaded = texture_import::load(&file, &import, kind, compress)?;
    Some(Decoded { import, loaded })
}
