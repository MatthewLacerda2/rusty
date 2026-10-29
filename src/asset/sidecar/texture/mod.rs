//! A texture's import settings (#1323), Unity's Texture Import Settings: how it
//! wraps, how it filters, whether it gets a mip chain, how it is compressed and
//! how large it may be (#1574). Stored in the image's
//! `.meta` sidecar beside the model settings, read by the renderer's upload
//! (`render::gpu::textures`), and written by the image inspector and
//! `Assets.SetTextureImportSettings` through [`set_texture`].

mod modes;

pub use modes::{Compression, FilterMode, TextureType, WrapMode};

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::{load, meta_path, save};
use crate::asset::ImportError;

/// The largest side a texture keeps by default (Unity's *Max Size*).
pub const DEFAULT_MAX_SIZE: u32 = 2048;
/// The *Max Size* choices, Unity's: a larger image is downscaled at import, its
/// aspect kept, until its longer side fits.
pub const MAX_SIZES: [u32; 9] = [32, 64, 128, 256, 512, 1024, 2048, 4096, 8192];

/// One texture's import settings. `#[serde(default)]` so a sparse sidecar loads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextureSettings {
    pub wrap_mode: WrapMode,
    pub filter_mode: FilterMode,
    /// Upload a full mip chain (Unity's *Generate Mip Maps*, on by default).
    pub generate_mipmaps: bool,
    /// What its texels hold, which picks its block format (#1574).
    pub texture_type: TextureType,
    /// How it is block-compressed (#1574).
    pub compression: Compression,
    /// The longest side it is uploaded at, one of [`MAX_SIZES`] (#1574).
    pub max_size: u32,
}

impl Default for TextureSettings {
    fn default() -> Self {
        Self {
            wrap_mode: WrapMode::default(),
            filter_mode: FilterMode::default(),
            generate_mipmaps: true,
            texture_type: TextureType::default(),
            compression: Compression::default(),
            max_size: DEFAULT_MAX_SIZE,
        }
    }
}

/// `size` when it is one of [`MAX_SIZES`], else an error listing them.
pub fn parse_max_size(size: u32) -> Result<u32, String> {
    if MAX_SIZES.contains(&size) {
        return Ok(size);
    }
    let known = MAX_SIZES.map(|s| s.to_string()).join(", ");
    Err(format!("max size {size} is not one of {known}"))
}

/// The import settings of the image at `source`: its sidecar's, or the defaults
/// when it has none or the sidecar does not parse.
/// A *Max Size* outside [`MAX_SIZES`] (a hand-edited sidecar) is warned of once
/// per file and read as [`DEFAULT_MAX_SIZE`], as the setter would refuse it.
pub fn texture(source: &Path) -> TextureSettings {
    let mut settings = load(source).map(|s| s.texture).unwrap_or_default();
    if let Err(why) = parse_max_size(settings.max_size) {
        warn_once(source, &why);
        settings.max_size = DEFAULT_MAX_SIZE;
    }
    settings
}

/// Files whose sidecar has been warned of.
static WARNED: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());

fn warn_once(source: &Path, why: &str) {
    let mut warned = WARNED.lock().unwrap_or_else(|e| e.into_inner());
    if warned.insert(source.to_path_buf()) {
        log::warn!(
            "[Import] {}: {why}; using {DEFAULT_MAX_SIZE}",
            meta_path(source).display()
        );
    }
}

/// Set the image at `source`'s import settings, keeping the rest of its sidecar.
/// The one setter the image inspector and `Assets.SetTextureImportSettings` share.
/// The write is noted in `core::fs::written`, so the renderer drops the uploaded
/// texture and re-uploads it with the new settings on the next frame. Its
/// compressed copy is re-made by the import (`asset::texture_import`): in the
/// background when the editor's watch sees the sidecar change, else by the
/// refresh before Play.
pub fn set_texture(source: &Path, settings: TextureSettings) -> Result<(), ImportError> {
    let mut stored = load(source)?;
    if stored.texture == settings {
        return Ok(());
    }
    stored.texture = settings;
    save(source, &stored)?;
    crate::core::fs::written::note_written(&meta_path(source).to_string_lossy());
    Ok(())
}

/// The image at `source`, just assigned to a material's normal slot, set to
/// [`TextureType::NormalMap`] when it is still `Default`, and said so: Unity's
/// *Fix now*, done for the agent (#1574). Only an image on disk, and only while
/// imports may change (never in Play, `convert::conversion_allowed`).
pub fn fit_normal_slot(source: &Path) {
    if !crate::asset::convert::conversion_allowed() || !source.is_file() {
        return;
    }
    let settings = texture(source);
    if settings.texture_type != TextureType::Default {
        return;
    }
    let fixed = TextureSettings {
        texture_type: TextureType::NormalMap,
        ..settings
    };
    match set_texture(source, fixed) {
        Ok(()) => log::info!(
            "[Import] {}: used as a normal map, so its Texture Type is now NormalMap",
            source.display()
        ),
        Err(e) => log::warn!("[Import] {}: {e}", source.display()),
    }
}

#[cfg(test)]
mod tests;
