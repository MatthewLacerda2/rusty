//! Unity's Texture Import Settings, in the image's `.meta` sidecar.

mod baked;
mod modes;

pub use baked::{check_bake_size, prepare_bake};
pub use modes::{Compression, FilterMode, TextureType, WrapMode};

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::{load, meta_path, save};
use crate::ImportError;

pub const DEFAULT_MAX_SIZE: u32 = 2048;
pub const MAX_SIZES: [u32; 9] = [32, 64, 128, 256, 512, 1024, 2048, 4096, 8192];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextureSettings {
    pub wrap_mode: WrapMode,
    pub filter_mode: FilterMode,
    pub generate_mipmaps: bool,
    pub texture_type: TextureType,
    /// *sRGB (Color Texture)*, `Default` only: off means data stored raw (a packed
    /// metallic/roughness map); `NormalMap` and `SingleChannel` are always data.
    pub srgb: bool,
    pub compression: Compression,
    pub max_size: u32,
    /// Whether the developer chose `max_size`; if not, a bake moves it ([`prepare_bake`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub max_size_chosen: bool,
}

impl TextureSettings {
    pub fn chosen(self, size: u32) -> Self {
        Self {
            max_size: size,
            max_size_chosen: true,
            ..self
        }
    }
}

impl Default for TextureSettings {
    fn default() -> Self {
        Self {
            wrap_mode: WrapMode::default(),
            filter_mode: FilterMode::default(),
            generate_mipmaps: true,
            texture_type: TextureType::default(),
            srgb: true,
            compression: Compression::default(),
            max_size: DEFAULT_MAX_SIZE,
            max_size_chosen: false,
        }
    }
}

pub fn parse_max_size(size: u32) -> Result<u32, String> {
    if MAX_SIZES.contains(&size) {
        return Ok(size);
    }
    let known = MAX_SIZES.map(|s| s.to_string()).join(", ");
    Err(format!("max size {size} is not one of {known}"))
}

pub fn texture(source: &Path) -> TextureSettings {
    read_texture(source).unwrap_or_default()
}

/// A *Max Size* outside [`MAX_SIZES`] (a hand edit) is warned of once per file and read as
/// [`DEFAULT_MAX_SIZE`].
pub fn read_texture(source: &Path) -> Result<TextureSettings, ImportError> {
    let extracted = || crate::convert::texture_ref::extracted::settings(source);
    let mut settings = match super::stored(source)? {
        Some(stored) => stored.texture,
        None => extracted().unwrap_or_default(),
    };
    if let Err(why) = parse_max_size(settings.max_size) {
        warn_once(source, &why);
        settings.max_size = DEFAULT_MAX_SIZE;
    }
    Ok(settings)
}

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

/// Refused for a texture a conversion extracted: its settings follow its model's slots
/// ([`texture`]).
pub fn set_texture(source: &Path, settings: TextureSettings) -> Result<(), ImportError> {
    if let Some(glb) = crate::convert::texture_ref::extracted::glb_of(source) {
        return Err(ImportError::Sidecar(format!(
            "{} is a texture extracted from a model ({}): its import settings follow \
             the slots the model uses it in and cannot be set",
            source.display(),
            glb.display()
        )));
    }
    let mut stored = load(source)?;
    if stored.texture == settings {
        return Ok(());
    }
    stored.texture = settings;
    save(source, &stored)
}

pub fn fit_normal_slot(source: &Path) {
    fit_slot(
        source,
        "a normal map, so its Texture Type is now NormalMap",
        |s| {
            (s.texture_type == TextureType::Default).then_some(TextureSettings {
                texture_type: TextureType::NormalMap,
                ..*s
            })
        },
    );
}

pub fn fit_data_slot(source: &Path) {
    fit_slot(
        source,
        "a data map (metallic, roughness, occlusion, specular level, detail), so its sRGB is now off",
        |s| {
            (s.texture_type == TextureType::Default && s.srgb)
                .then_some(TextureSettings { srgb: false, ..*s })
        },
    );
}

pub fn fit_single_channel_slot(source: &Path, used: &str) {
    fit_slot(
        source,
        &format!("{used}, so its Texture Type is now SingleChannel"),
        |s| {
            (s.texture_type == TextureType::Default).then_some(TextureSettings {
                texture_type: TextureType::SingleChannel,
                ..*s
            })
        },
    );
}

fn fit_slot(source: &Path, used: &str, fix: impl Fn(&TextureSettings) -> Option<TextureSettings>) {
    if !engine_may_write(source) {
        return;
    }
    let Some(fixed) = fix(&texture(source)) else {
        return;
    };
    match set_texture(source, fixed) {
        Ok(()) => log::info!("[Import] {}: used as {used}", source.display()),
        Err(e) => log::warn!("[Import] {}: {e}", source.display()),
    }
}

fn engine_may_write(source: &Path) -> bool {
    source.is_file() && engine_may_write_beside(source)
}

fn engine_may_write_beside(source: &Path) -> bool {
    use crate::convert::{conversion_allowed, is_import_cache};
    conversion_allowed() && !is_import_cache(source)
}

#[cfg(test)]
mod tests;
