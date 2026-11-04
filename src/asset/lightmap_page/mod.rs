//! src/asset/lightmap_page/ — a baked lightmap page as an asset (#1694), Unity's
//! *Lightmap* texture type.
//!
//! A lightmap bake writes its pages into one folder beside its scene,
//! `<scene file>`[`DIR_SUFFIX`] (`dev::bake::lightmap::lightmap_dir`). Those pages
//! are not colour textures: the generic texture importer (`texture_import`) leaves
//! every file under such a folder alone ([`is_baked`]), so a page never gets a BC7
//! sRGB cache nothing reads, and the renderer loads the page the bake wrote.
//!
//! **What a page is** ([`Page`]): one square level chain, block-compressed, in a
//! KTX2 file (`container`). Irradiance is BC6H unsigned float ([`PageFormat::Bc6h`], a
//! byte a texel, HDR stored as is: no RGBM, so bilinear filtering is linear in the
//! light); a directional bake's direction page is BC7 ([`PageFormat::Bc7`]). The
//! bake (`scene::lighting::lightmap`) makes the levels; this module only encodes,
//! decodes and stores them (`blocks`), as Unity, Godot 4 and Unreal store theirs.

mod blocks;
mod container;

use std::path::Path;

pub use blocks::{decode_bc6h, decode_bc7, encode_bc6h, encode_bc7, to_rgba16f};
pub use container::{read, write};

/// The suffix of the folder beside a scene file that holds its baked lightmap
/// pages: `foo.scene` → `foo.scene.lightmaps/`.
pub const DIR_SUFFIX: &str = ".lightmaps";

/// Whether `path` lies in a scene's lightmap folder (a folder whose name ends in
/// [`DIR_SUFFIX`]): a baked page the texture importer leaves alone. The one check
/// every caller asks.
pub fn is_baked(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    parent.components().any(|c| match c {
        std::path::Component::Normal(name) => name
            .to_str()
            .is_some_and(|n| n.len() > DIR_SUFFIX.len() && n.ends_with(DIR_SUFFIX)),
        _ => false,
    })
}

/// A page's block format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageFormat {
    /// Unsigned-float HDR RGB: irradiance.
    Bc6h,
    /// RGBA8 unorm: a direction page.
    Bc7,
}

impl PageFormat {
    pub const ALL: [Self; 2] = [Self::Bc6h, Self::Bc7];

    /// Its `VkFormat`, what a KTX2 header names.
    pub fn vk_format(self) -> u32 {
        match self {
            Self::Bc6h => 143, // VK_FORMAT_BC6H_UFLOAT_BLOCK
            Self::Bc7 => 145,  // VK_FORMAT_BC7_UNORM_BLOCK
        }
    }

    /// The bytes of one `edge`² level: 16 a 4×4 block.
    pub fn level_len(self, edge: u32) -> usize {
        let blocks = edge.div_ceil(4) as usize;
        blocks * blocks * 16
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
        .take_while(|&l| edge >> l >= 4 && (edge >> l) % 4 == 0)
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
            "assets/a.scene.lightmaps/deeper/page.png",
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
        ];
        for path in not {
            assert!(!is_baked(Path::new(path)), "{path}");
        }
    }
}
