use std::path::Path;

use super::{compressed, GpuTexture, TextureKind};
use crate::{Renderer, Uploads};
use rusty_asset::texture_import::{self, fits};
use rusty_asset::MapRef;

impl Renderer {
    pub fn uncached_textures(&self, root: &Path) -> Uploads {
        let mut found = Uploads::default();
        for (map, tex) in &self.gpu_textures {
            if self.wants_cache(root, map, tex) {
                found.add(compressed::gpu_bytes(&tex.texture));
            }
        }
        found
    }

    fn wants_cache(&self, root: &Path, map: &MapRef, tex: &GpuTexture) -> bool {
        if tex.texture.format().is_compressed() || tex.deep_normal {
            return false;
        }
        let file = map.file().filter(|f| f.is_file());
        let Some(file) = file.filter(|f| texture_import::imports(root, f)) else {
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
