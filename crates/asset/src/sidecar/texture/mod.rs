//! A texture's import settings (#1323), Unity's Texture Import Settings: how it
//! wraps, how it filters, whether it gets a mip chain, how it is compressed and
//! how large it may be (#1574). Stored in the image's
//! `.meta` sidecar beside the model settings, read by the renderer's upload
//! (`render::gpu::textures`), and written by the image inspector and
//! `Assets.SetTextureImportSettings` through [`set_texture`]. A Max Size the
//! engine chose follows the size of the latest bake; one the developer set is
//! never changed by a bake (`baked`).

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
    /// Unity's *sRGB (Color Texture)*, read for a `Default` texture only: on, its
    /// texels are sRGB colour (mips average in linear light, the colour view
    /// decodes); off, they are data stored raw (a packed metallic/roughness map),
    /// averaged and sampled as stored. `NormalMap` and `SingleChannel` are always
    /// data.
    pub srgb: bool,
    /// How it is block-compressed (#1574).
    pub compression: Compression,
    /// The longest side it is uploaded at, one of [`MAX_SIZES`] (#1574).
    pub max_size: u32,
    /// Whether the developer chose `max_size` (the inspector's combo,
    /// `Assets.SetTextureImportSettings`, a hand edit that also sets this key).
    /// Only a deliberate act sets it, so a missing key (every sidecar the engine
    /// writes, a sparse hand-written block) means the engine's Max Size, which a
    /// bake moves to its size ([`prepare_bake`]); a chosen one no bake touches. Written to
    /// the file only when true.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub max_size_chosen: bool,
}

impl TextureSettings {
    /// These settings with Max Size `size`, set by the developer (the inspector,
    /// `Assets.SetTextureImportSettings`), so no bake changes it.
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

/// `size` when it is one of [`MAX_SIZES`], else an error listing them.
pub fn parse_max_size(size: u32) -> Result<u32, String> {
    if MAX_SIZES.contains(&size) {
        return Ok(size);
    }
    let known = MAX_SIZES.map(|s| s.to_string()).join(", ");
    Err(format!("max size {size} is not one of {known}"))
}

/// The import settings of the image at `source`: its sidecar's, or the defaults
/// when it has none or the sidecar does not parse ([`read_texture`] says which).
pub fn texture(source: &Path) -> TextureSettings {
    read_texture(source).unwrap_or_default()
}

/// [`texture`] from one read of the sidecar, with an error when it exists and
/// does not parse: what `Assets.GetTextureImportSettings` reports. A texture a
/// conversion extracted with no sidecar takes the defaults typed by the slots its
/// model samples it in (`convert::texture_ref::extracted`, #2365).
/// A *Max Size* outside [`MAX_SIZES`] (a hand-edited sidecar) is warned of once
/// per file and read as [`DEFAULT_MAX_SIZE`], as the setter would refuse it.
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
///
/// A texture a conversion extracted is refused: its settings follow the slots
/// its model samples it in ([`texture`]), and a sidecar in the import cache
/// would go with the next conversion's folder. Unity's embedded textures are
/// read-only the same way until they are extracted into the project.
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

/// The image at `source`, just assigned to a material's normal slot, set to
/// [`TextureType::NormalMap`] when it is still `Default`, and said so: Unity's
/// *Fix now*, done for the agent (#1574).
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

/// The image at `source`, just assigned to one of a material's data slots
/// (metallic, roughness, occlusion, specular level, detail), set to linear (*sRGB* off) when it is a `Default` texture still marked
/// sRGB, and said so: its texels are data, so its mips average raw, as an
/// uncompressed upload in that slot does (#1322, #1574).
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

/// The image at `source`, just assigned to a slot whose shader reads its red alone,
/// raw (a height map's parallax march, a detail mask), set to
/// [`TextureType::SingleChannel`] when it is still `Default`, as Unity's *Single
/// Channel* keeps it, and said so: `used` names the slot ("a height map").
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

/// Apply `fix` to the import settings of `source` when it asks for a change, and
/// log it as `used` (what it was used as, and what changed). Only an image on disk, and only while imports may change (never
/// in Play, `convert::conversion_allowed`). A texture a converted model's cache
/// holds is left alone: its settings follow the slots its model's glTF samples
/// it in ([`texture`]), and its folder is re-made by the next conversion.
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

/// Whether the engine may change the sidecar of the image at `source` on its own
/// (a slot fix): only an image on disk, outside a converted model's cache, and
/// only while imports may change (never in Play).
fn engine_may_write(source: &Path) -> bool {
    source.is_file() && engine_may_write_beside(source)
}

/// [`engine_may_write`] without the file on disk: a bake writes the sidecar of
/// the image it is about to write.
fn engine_may_write_beside(source: &Path) -> bool {
    use crate::convert::{conversion_allowed, is_import_cache};
    conversion_allowed() && !is_import_cache(source)
}

#[cfg(test)]
mod tests;
