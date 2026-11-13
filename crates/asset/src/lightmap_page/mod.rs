//! crates/asset/src/lightmap_page/ — a baked lightmap page as an asset (#1694), Unity's
//! *Lightmap* texture type.
//!
//! A lightmap bake writes its pages into one folder beside its scene,
//! `<scene file>`[`DIR_SUFFIX`] (`dev::bake::lightmap::lightmap_dir`). Those pages
//! are not colour textures: the generic texture importer (`texture_import`) leaves
//! every file under such a folder alone ([`is_baked`]), so a page never gets a BC7
//! sRGB cache nothing reads, and the renderer loads the page the bake wrote.
//!
//! **What a page is** ([`Page`]): one square level chain in a KTX2 file
//! (`container`). Irradiance is BC6H unsigned float ([`PageFormat::Bc6h`], a
//! byte a texel, HDR stored as is: no RGBM, so bilinear filtering is linear in the
//! light). A directional bake's direction page is plain RGBA8
//! ([`PageFormat::Rgba8`], four bytes a texel), never block-compressed: BC7 turned
//! a lamp's direction up to 5° off a texel, which a glossy surface's baked
//! highlight showed as a sawtooth (#1996). Its mips are the bake's, made from its
//! float directions, so the loader uploads the levels as they are. The
//! bake (`scene::lighting::lightmap`) makes the levels; this module only encodes,
//! decodes and stores them (`blocks`), as Unity, Godot 4 and Unreal store theirs.
//! Baked reflection cubes share the BC6H level encoder ([`bc6h_level`], #1885).
//!
//! stored-format: the baked lightmap pages in `<scene>.lightmaps/`

mod blocks;
mod container;

use std::path::Path;

pub use blocks::{bc6h_level, bc6h_texels, decode_bc6h, encode_bc6h, to_rgba16f};
pub use container::{read, write};

/// The suffix of the folder beside a scene file that holds its baked lightmap
/// pages: `foo.scene` → `foo.scene.lightmaps/`.
pub const DIR_SUFFIX: &str = ".lightmaps";

/// Whether `path` lies directly in a scene's lightmap folder (its parent's name
/// ends in [`DIR_SUFFIX`]): a baked page the texture importer leaves alone. The one
/// check every caller asks. Only the direct parent counts, so a project kept under
/// a folder that happens to end in `.lightmaps` is still imported.
pub fn is_baked(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.len() > DIR_SUFFIX.len() && n.ends_with(DIR_SUFFIX))
}

/// A page's texel format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageFormat {
    /// Unsigned-float HDR RGB, block-compressed: irradiance.
    Bc6h,
    /// Uncompressed linear RGBA8: a direction page (#810, #1996), its bytes
    /// `lightmap::encode_direction`'s.
    Rgba8,
}

impl PageFormat {
    pub const ALL: [Self; 2] = [Self::Bc6h, Self::Rgba8];

    /// Its `VkFormat`, what a KTX2 header names.
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

/// One baked page, every level, largest first.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub format: PageFormat,
    pub edge: u32,
    pub levels: Vec<Vec<u8>>,
}

impl Page {
    /// The bytes it takes on the GPU, every level.
    pub fn gpu_bytes(&self) -> usize {
        self.levels.iter().map(Vec::len).sum()
    }
}

/// The most levels an `edge`² page has whose edges are each a whole number of
/// blocks: what a page may hold.
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
