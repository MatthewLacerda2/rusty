//! Which resident textures were uploaded uncompressed only for want of a current
//! import cache (#2319): a headless run on a project the editor never imported
//! draws them as RGBA8, four bytes a texel against BC's one or half, so its memory
//! and load are not the shipped game's. `bench-scenes` warns with this.

use super::{compressed, GpuTexture, TextureKind};
use crate::{Renderer, Uploads};
use rusty_asset::texture_import::{self, fits};
use rusty_asset::MapRef;

impl Renderer {
    /// The textures in the material cache uploaded uncompressed though their
    /// import settings ask for compression, this GPU has BC, their Texture Type
    /// fits the slot of their first use, and an import would make them a cache
    /// (no 16-bit normal map, sides a multiple of 4): how many, and their GPU
    /// bytes with every mip. A render texture or a file gone is not counted.
    pub fn uncached_textures(&self) -> Uploads {
        let mut found = Uploads::default();
        for (map, tex) in &self.gpu_textures {
            if self.wants_cache(map, tex) {
                found.add(compressed::gpu_bytes(&tex.texture));
            }
        }
        found
    }

    fn wants_cache(&self, map: &MapRef, tex: &GpuTexture) -> bool {
        if tex.texture.format().is_compressed() || tex.deep_normal {
            return false;
        }
        let Some(file) = map.file().filter(|f| f.is_file()) else {
            return false;
        };
        let import = rusty_asset::sidecar::texture(&file);
        let kind = self.texture_kinds.kind_of(map.as_str());
        let kind = kind.unwrap_or(TextureKind::Colour);
        let (w, h) = (tex.texture.width(), tex.texture.height());
        texture_import::format_of(&import).is_some()
            && fits(import.texture_type, kind)
            && compressed::wanted(self.device.features(), &import)
            && w % 4 == 0
            && h % 4 == 0
    }
}
