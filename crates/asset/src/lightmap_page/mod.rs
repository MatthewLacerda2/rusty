//! Unity's *Lightmap* texture type. Irradiance is BC6H, HDR stored as is (no
//! RGBM, so bilinear filtering is linear in the light). A direction page is
//! RGBA8, never block-compressed: BC7 turned a lamp's direction up to 5° off a
//! texel, a sawtooth in glossy highlights.
//!
//! stored-format: the baked lightmap pages in `<scene>.lightmaps/`

mod blocks;
mod container;

use std::path::Path;

pub use blocks::{bc6h_level, bc6h_texels, decode_bc6h, encode_bc6h, to_rgba16f};
pub use container::{read, write};

/// `foo.scene` → `foo.scene.lightmaps/`; the texture importer skips it.
pub const DIR_SUFFIX: &str = ".lightmaps";

/// Only the direct parent counts.
pub fn is_baked(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.len() > DIR_SUFFIX.len() && n.ends_with(DIR_SUFFIX))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageFormat {
    Bc6h,
    /// A direction page, its bytes `lightmap::encode_direction`'s.
    Rgba8,
}

impl PageFormat {
    pub const ALL: [Self; 2] = [Self::Bc6h, Self::Rgba8];

    pub fn vk_format(self) -> u32 {
        match self {
            Self::Bc6h => 143, // VK_FORMAT_BC6H_UFLOAT_BLOCK
            Self::Rgba8 => 37, // VK_FORMAT_R8G8B8A8_UNORM
        }
    }

    /// The bytes of one `edge`² level: 16 a 4×4 block, or 4 a texel.
    pub fn level_len(self, edge: u32) -> usize {
        match self {
            Self::Bc6h => crate::texture_import::blocks_len(edge, edge, 16),
            Self::Rgba8 => (edge as usize).pow(2) * 4,
        }
    }
}

/// Levels largest first.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub format: PageFormat,
    pub edge: u32,
    pub levels: Vec<Vec<u8>>,
}

impl Page {
    pub fn gpu_bytes(&self) -> usize {
        self.levels.iter().map(Vec::len).sum()
    }
}

/// Levels whose edges are each a whole number of blocks.
pub fn max_levels(edge: u32) -> u32 {
    (0..32)
        .take_while(|&l| edge >> l >= 4 && (edge >> l).is_multiple_of(4))
        .count() as u32
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod name_tests {
    use super::*;

    #[test]
    fn pages_under_a_scene_lightmap_folder_are_baked_and_nothing_else_is() {
        let baked = [
            "assets/scenes/kolvik.scene.lightmaps/lightmap_0_ab12cd34.png",
            "/work/old.lightmaps/assets/a.scene.lightmaps/page.ktx2",
        ];
        for path in baked {
            assert!(is_baked(Path::new(path)), "{path}");
        }
        let not = [
            "assets/textures/floor.png",
            "assets/scenes/kolvik.scene.lightmaps",
            "assets/.lightmaps/page.png",
            "assets/lightmaps/page.png",
            "assets/my.lightmaps.png",
            "assets/a.scene.lightmaps/deeper/page.png",
            "/work/old.lightmaps/assets/textures/floor.png",
        ];
        for path in not {
            assert!(!is_baked(Path::new(path)), "{path}");
        }
    }
}
