//! src/asset/lut/ — user colour-lookup tables (#1441): what a colourist grades in
//! Photoshop, DaVinci Resolve or Affinity and exports, read into the engine's one
//! form, a [`Lut`]: an `N³` table of display colours, red fastest.
//!
//! Two sources, the ones Unity's *Color Lookup* takes ([`LUT_EXTENSIONS`]):
//!
//! - **`.cube`**, Adobe's Cube LUT format (`cube`), the de facto standard: a 3D
//!   table of `2` to [`Lut::MAX_SIZE`] texels a side, with its `DOMAIN_MIN` /
//!   `DOMAIN_MAX`.
//! - **A strip `.png`** (`strip`), Unity's 2D layout, common in asset packs: `N`
//!   slices of `N × N` side by side, `N² × N` pixels (`256×16`, `1024×32`).
//!
//! The renderer uploads a [`Lut`] as a 3D texture, caches it per path and re-reads
//! the file when it is written (`render::postfx::effects::grading::color_lookup::user`).
//! A parse is a few milliseconds even at 65³, so no converted copy is kept on disk:
//! like a `.png` texture, the source is what loads, in the editor and in a shipped
//! game.

mod cube;
mod strip;

use std::path::Path;

use super::{ImportError, LUT_EXTENSIONS};
use crate::core::paths;

/// A 3D colour lookup: `size³` RGBA texels (alpha `1`), red fastest, then green,
/// then blue, holding the display colour each input maps to.
#[derive(Clone, Debug, PartialEq)]
pub struct Lut {
    /// Texels per side.
    pub size: u32,
    /// The input range the table spans: `[min, max]`, per channel (`DOMAIN_MIN` /
    /// `DOMAIN_MAX`; `[0, 1]` for a strip).
    pub domain: [[f32; 3]; 2],
    pub texels: Vec<[f32; 4]>,
}

impl Lut {
    /// The largest table read, a side: Resolve's 65³ fits, and a 256³ table
    /// (268 MB uploaded) is refused rather than allocated.
    pub const MAX_SIZE: u32 = 65;
    /// The `.cube` spec's smallest table.
    pub const MIN_SIZE: u32 = 2;

    /// Whether `size` is a side this importer reads.
    fn fits(size: u32) -> bool {
        (Self::MIN_SIZE..=Self::MAX_SIZE).contains(&size)
    }
}

/// Read the colour lookup at `path`, by its extension: one [`LUT_EXTENSIONS`]
/// names, a `.cube` read as text and any other as a strip image. The error names
/// the file, and for a `.cube` the line.
pub fn load(path: &Path) -> Result<Lut, ImportError> {
    let named = |why: String| ImportError::Parse(format!("{}{why}", path.display()));
    let ext = paths::extension(path);
    if !LUT_EXTENSIONS.contains(&ext.as_str()) {
        return Err(ImportError::UnsupportedFormat(format!(
            "{}: .{ext} is not a colour lookup (.{})",
            path.display(),
            LUT_EXTENSIONS.join(", .")
        )));
    }
    if ext == CUBE {
        // Lossily: a `TITLE` in Latin-1 must not refuse the table.
        let bytes = std::fs::read(path).map_err(|e| named(format!(": {e}")))?;
        let text = String::from_utf8_lossy(&bytes);
        return cube::parse(&text).map_err(|(line, why)| named(format!(":{line}: {why}")));
    }
    let image = image::open(path).map_err(|e| named(format!(": {e}")))?;
    let rgba = image.to_rgba8();
    strip::from_rgba8(rgba.width(), rgba.height(), rgba.as_raw())
        .map_err(|why| named(format!(": {why}")))
}

/// The one [`LUT_EXTENSIONS`] entry read as text; every other is a strip image.
const CUBE: &str = "cube";

#[cfg(test)]
pub(crate) mod tests;
