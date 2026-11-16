//! A baked image's *Max Size* (#2259): a bake knows the size it writes, so the
//! game ships what the recipe asked for. A Max Size the engine chose (no sidecar,
//! a slot fix's default, an earlier bake's) follows the latest bake; one the
//! developer set (the inspector, `Assets.SetTextureImportSettings`, a hand-edited
//! `.meta`) is never changed by a bake, and the import warns once that a larger
//! file is shrunk (`texture_import`). A bake larger than the largest Max Size is
//! refused before it writes ([`check_bake_size`]).

use std::path::Path;

use super::{engine_may_write, load, save, texture, DEFAULT_MAX_SIZE, MAX_SIZES};
use crate::ImportError;

/// The largest Max Size the importer takes.
const LARGEST: u32 = MAX_SIZES[MAX_SIZES.len() - 1];

/// Refuse a `width`×`height` bake whose longer side is above the largest Max Size
/// (8192): the import would shrink it and no setting could keep it. Called by
/// every texture bake verb before it writes.
pub fn check_bake_size(width: u32, height: u32) -> Result<(), String> {
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

/// The image a bake just wrote at `source`, kept at the size it was baked at when
/// its Max Size is the engine's ([`super::TextureSettings::max_size_is_engines`]):
/// Max Size moves to the smallest of [`MAX_SIZES`] at or above its longer side,
/// never below the default 2048, so a recipe's `resolution` is what the game gets.
/// A developer's Max Size is never touched. Nothing is written when it already
/// fits, while conversion is refused (Play), or for an image in the import cache.
/// Returns whether it wrote the sidecar.
pub fn keep_baked_size(source: &Path) -> Result<bool, ImportError> {
    if !engine_may_write(source) {
        return Ok(false);
    }
    let settings = texture(source);
    if !settings.max_size_is_engines() {
        return Ok(false);
    }
    let (w, h) =
        image::image_dimensions(source).map_err(|e| ImportError::Sidecar(e.to_string()))?;
    let max_size = fitting(w.max(h));
    if max_size == settings.max_size {
        return Ok(false);
    }
    let mut stored = load(source)?;
    stored.texture.max_size = max_size;
    stored.texture.engine_max_size = Some(max_size);
    save(source, &stored)?;
    Ok(true)
}

/// [`keep_baked_size`] for each image `verb` just baked: a bake stands when its
/// sidecar cannot be written, warned of, and imports at the Max Size it had.
pub fn keep_baked_sizes<'a>(verb: &str, baked: impl IntoIterator<Item = &'a str>) {
    for path in baked {
        if let Err(e) = keep_baked_size(Path::new(path)) {
            log::warn!("[Import] {verb}: {path}: cannot keep its baked size in its .meta: {e}");
        }
    }
}

#[cfg(test)]
#[path = "baked_tests.rs"]
mod tests;
