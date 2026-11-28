use std::path::Path;

use super::{compressed, GpuTexture, TextureKind};
use crate::{Renderer, Uploads};
use rusty_asset::texture_import::{self, fits};
use rusty_asset::MapRef;

impl Renderer {
    /// Textures uploaded RGBA8 only for want of a current import cache: a headless run's
    /// memory and load are then not the shipped game's. A streamed texture counts as loaded,
    /// at its full size and every level, not the levels it keeps now.
    pub fn uncached_textures(&self, root: &Path) -> Uploads {
        let mut found = Uploads::default();
        for (map, tex) in &self.gpu_textures {
            if self.wants_cache(root, map, tex) {
                found.add(loaded(tex).1);
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
        let ((w, h), _) = loaded(tex);
        texture_import::format_of(&import).is_some()
            && fits(import.texture_type, kind)
            && compressed::wanted(self.device.features(), &import)
            // BC blocks are 4×4 texels.
            && w % 4 == 0
            && h % 4 == 0
    }
}

/// The size and bytes of the texture as the file loads it, before streaming drops a level.
fn loaded(tex: &GpuTexture) -> ((u32, u32), u64) {
    match tex.mips {
        Some(mips) => (
            mips.full_size,
            mips.level_bytes(tex.texture.format()).iter().sum(),
        ),
        None => {
            let size = (tex.texture.width(), tex.texture.height());
            (size, compressed::gpu_bytes(&tex.texture))
        }
    }
}
