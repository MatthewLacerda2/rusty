//! Loading a texture by the map that names it: the path-keyed cache, its misses
//! and the uncompressed copies a slot needs (#689, #1574), and the one upload of an
//! image file, compressed from the import cache or decoded and fitted.

use std::rc::Rc;

use image::GenericImageView;

use super::{compressed, sampler, upload, MipSettings, TextureKind, DATA_FORMAT};
use crate::asset::sidecar::{TextureSettings, TextureType};
use crate::asset::texture_import::{self, Texels};
use crate::asset::MapRef;
use crate::render::{GpuTexture, Renderer};

impl Renderer {
    /// Loads and registers the texture `map` names. Caches results dynamically,
    /// keyed by the map; a failed load is remembered as a miss (drawn with the
    /// default) and retried once the file appears, never cached under its path (#689).
    /// A colour texture; `load_texture_as` names what the texels hold.
    pub fn load_texture(&mut self, map: &MapRef) -> Rc<GpuTexture> {
        self.load_texture_as(map, TextureKind::Colour)
    }

    /// [`Self::load_texture`] for a texture holding `kind`, which decides how an
    /// uncompressed upload's mips average. The first load of a file decides it:
    /// the cache keeps one upload per file, and a later request of the other kind
    /// warns once ([`kind_watch::KindWatch`]). A compressed upload a later slot
    /// cannot read (`compressed::reads_as`) is joined by an uncompressed copy
    /// for it, so no use loses a channel (#1574).
    pub(crate) fn load_texture_as(&mut self, map: &MapRef, kind: TextureKind) -> Rc<GpuTexture> {
        let path_str = map.as_str();
        if let Some(tex) = self.gpu_textures.get(path_str) {
            self.texture_kinds.requested(path_str, kind, "material");
            if compressed::reads_as(&tex.texture, kind) {
                return Rc::clone(tex);
            }
            return self.fallback(map, kind);
        }
        if self.texture_freshness.is_miss(path_str) {
            return Rc::clone(&self.default_texture);
        }
        // A render texture (#430) is never on disk: it is registered by the frame
        // that draws it, and until then shows the default — uncached, so it appears
        // the moment its camera exists.
        if crate::render::render_texture::is_render_texture(path_str) {
            return Rc::clone(&self.default_texture);
        }

        let Some(tex) = self.upload_map(map, kind) else {
            self.texture_freshness.miss(path_str);
            return Rc::clone(&self.default_texture);
        };
        self.texture_kinds.uploaded(path_str, kind);
        self.gpu_textures.insert(map.clone(), Rc::clone(&tex));
        tex
    }

    /// The texture `map` names for a slot holding `kind`, when its cached upload is
    /// one [`Self::load_texture_as`] already made: that upload, or its
    /// uncompressed copy where the slot cannot read its format. What a material
    /// bind group binds.
    pub(crate) fn texture_for(&self, map: &MapRef, kind: TextureKind) -> Option<Rc<GpuTexture>> {
        let tex = self.gpu_textures.get(map)?;
        match compressed::reads_as(&tex.texture, kind) {
            true => Some(Rc::clone(tex)),
            false => self.texture_fallbacks.get(map).cloned(),
        }
    }

    /// The uncompressed copy of `map` for `kind`, made on first need.
    fn fallback(&mut self, map: &MapRef, kind: TextureKind) -> Rc<GpuTexture> {
        if let Some(tex) = self.texture_fallbacks.get(map) {
            return Rc::clone(tex);
        }
        let Some(tex) = self.upload_map(map, kind) else {
            return Rc::clone(&self.default_texture);
        };
        self.texture_fallbacks.insert(map.clone(), Rc::clone(&tex));
        tex
    }

    /// Upload the image `map` names, uncached, with the mips, compression, size
    /// and sampler its import settings ask for; `None` when it cannot be read. A
    /// cache with its own keys (bloom's lens dirt, #1442) loads through this.
    /// Compressed, its levels come from the import cache (#1574), when it has a
    /// current one and its type fits `kind`; else it is decoded, fitted to its
    /// *Max Size* and its mips drawn on the GPU.
    pub(crate) fn upload_map(&self, map: &MapRef, kind: TextureKind) -> Option<Rc<GpuTexture>> {
        let file = map.file()?;
        let import = crate::asset::sidecar::texture(&file);
        let compress = compressed::wanted_for(map.as_str(), self.device.features(), &import, kind);
        let loaded = texture_import::load(&file, &import, kind, compress)?;
        let srgb = !texture_import::is_linear(&import);
        let mut texture = match &loaded.texels {
            Texels::Blocks(blocks) => {
                let gpu = (&self.device, &self.queue);
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
        Some(Rc::new(texture))
    }

    /// Uploads a decoded image holding `kind`, with the mips and the sampler its
    /// import settings ask for (#1323).
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
        let gpu = (&self.device, &self.queue);
        let srgb = !texture_import::is_linear(import);
        let texture = upload(gpu, path_str, &img.to_rgba8(), (levels, srgb));
        let kind = settings.kind;
        self.mip_generator
            .generate(&self.device, &self.queue, &texture, kind);
        Self::finalize_texture(
            &self.device,
            &self.texture_layout,
            (texture, DATA_FORMAT),
            (sampler(&self.device, import), import.filter_mode),
            Some(path_str),
        )
    }
}
