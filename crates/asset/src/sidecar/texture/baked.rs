//! A bake sets each image's Max Size to fit its size unless the developer chose it
//! (`TextureSettings::max_size_chosen`); the sidecar is written before the PNG, so an import
//! between the two never sees the new file at the old Max Size.

use std::path::Path;

use super::{engine_may_write_beside, load, save, texture, DEFAULT_MAX_SIZE, MAX_SIZES};
use crate::ImportError;

const LARGEST: u32 = MAX_SIZES[MAX_SIZES.len() - 1];

/// Refuses a bake whose longer side is above 8192, which no setting could keep. A sidecar that
/// cannot be written is warned of, and the bake goes on.
pub fn prepare_bake<'a>(
    verb: &str,
    (width, height): (u32, u32),
    paths: impl IntoIterator<Item = &'a str>,
) -> Result<(), String> {
    check_bake_size((width, height))?;
    let max_size = fitting(width.max(height));
    for path in paths {
        if let Err(e) = keep_baked_size(Path::new(path), max_size) {
            log::warn!("[Import] {verb}: {path}: cannot keep its baked size in its .meta: {e}");
        }
    }
    Ok(())
}

/// Called first by a verb, before it makes any folder, so a refused bake leaves the project
/// untouched.
pub fn check_bake_size((width, height): (u32, u32)) -> Result<(), String> {
    let longer = width.max(height);
    if longer <= LARGEST {
        return Ok(());
    }
    Err(format!(
        "a {width}x{height} bake is larger than the largest Max Size, {LARGEST}, so \
         the import would shrink it; bake at {LARGEST} or less"
    ))
}

fn fitting(longer: u32) -> u32 {
    let fit = MAX_SIZES.into_iter().find(|&s| s >= longer);
    fit.unwrap_or(LARGEST).max(DEFAULT_MAX_SIZE)
}

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
