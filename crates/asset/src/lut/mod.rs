//! As Unity's *Color Lookup* takes: `.cube`, or a strip `.png` (`N` slices of
//! `N × N` side by side, e.g. `256×16`). Parsing takes milliseconds even at 65³,
//! so no converted copy is kept.

mod cube;
mod strip;

use std::path::Path;

use super::{ImportError, LUT_EXTENSIONS};
use rusty_core::paths;

/// `size³` display-colour RGBA texels (alpha `1`), red fastest, then green, then blue.
#[derive(Clone, Debug, PartialEq)]
pub struct Lut {
    pub size: u32,
    /// `[min, max]` per channel; `[0, 1]` for a strip.
    pub domain: [[f32; 3]; 2],
    pub texels: Vec<[f32; 4]>,
}

impl Lut {
    /// Resolve's 65³ fits; a 256³ table (268 MB uploaded) is refused.
    pub const MAX_SIZE: u32 = 65;
    /// The `.cube` spec's smallest side.
    pub const MIN_SIZE: u32 = 2;

    fn fits(size: u32) -> bool {
        (Self::MIN_SIZE..=Self::MAX_SIZE).contains(&size)
    }
}

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

/// The one [`LUT_EXTENSIONS`] entry read as text; any other is read as a strip image.
const CUBE: &str = "cube";

#[cfg(test)]
mod tests;
