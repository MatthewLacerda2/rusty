//! A baked image's *Max Size* (#2259): a bake knows the size it writes, so the
//! game ships what the recipe asked for. Every texture bake verb calls
//! [`prepare_bake`] once, before it writes: a bake larger than the largest Max
//! Size is refused, and each image's Max Size is set to fit the bake's size
//! unless the developer chose it (`TextureSettings::max_size_chosen`). The
//! sidecar is written before the PNG, so an import between the two never sees
//! the new file at the old Max Size. A chosen Max Size smaller than the file
//! is warned of once on import (`texture_import`).

use std::path::Path;

use super::{engine_may_write_beside, load, save, texture, DEFAULT_MAX_SIZE, MAX_SIZES};
use crate::ImportError;

/// The largest Max Size the importer takes.
const LARGEST: u32 = MAX_SIZES[MAX_SIZES.len() - 1];

/// Ready the `width`×`height` images a bake as `verb` is about to write at
/// `paths`: refuse the bake when its longer side is above the largest Max Size
/// (8192), which no setting could keep, else move each engine-chosen Max Size to
/// fit it. The one call every texture bake verb makes, before it writes. A
/// sidecar that cannot be written is warned of, and the bake goes on.
pub fn prepare_bake<'a>(
    verb: &str,
    (width, height): (u32, u32),
    paths: impl IntoIterator<Item = &'a str>,
) -> Result<(), String> {
    check_bake_size(width, height)?;
    let max_size = fitting(width.max(height));
    for path in paths {
        if let Err(e) = keep_baked_size(Path::new(path), max_size) {
            log::warn!("[Import] {verb}: {path}: cannot keep its baked size in its .meta: {e}");
        }
    }
    Ok(())
}

/// Refuse a `width`×`height` bake whose longer side is above the largest Max Size.
fn check_bake_size(width: u32, height: u32) -> Result<(), String> {
    let longer = width.max(height);
    if longer <= LARGEST {
        return Ok(());
    }
    Err(format!(
        "a {width}x{height} bake is larger than the largest Max Size, {LARGEST}, so \
         the import would shrink it; bake at {LARGEST} or less"
    ))
}

/// The Max Size a bake whose longer side is `longer` ships at: the smallest of
/// [`MAX_SIZES`] at or above it, never below the default.
fn fitting(longer: u32) -> u32 {
    let fit = MAX_SIZES.into_iter().find(|&s| s >= longer);
    fit.unwrap_or(LARGEST).max(DEFAULT_MAX_SIZE)
}

/// Set the Max Size of the image about to be baked at `source` to `max_size`,
/// unless the developer chose its Max Size. Nothing is written when it already
/// holds it (so a 2048 bake with no sidecar gets none), while conversion is
/// refused (Play), or for an image in the import cache. Returns whether it wrote.
fn keep_baked_size(source: &Path, max_size: u32) -> Result<bool, ImportError> {
    if !engine_may_write_beside(source) {
        return Ok(false);
    }
    let settings = texture(source);
    if settings.max_size_chosen || settings.max_size == max_size {
        return Ok(false);
    }
    let mut stored = load(source)?;
    stored.texture.max_size = max_size;
    save(source, &stored)?;
    Ok(true)
}

#[cfg(test)]
#[path = "baked_tests.rs"]
mod tests;
