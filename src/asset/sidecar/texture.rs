//! A texture's import settings (#1323), Unity's Texture Import Settings: how it
//! wraps, how it filters, and whether it gets a mip chain. Stored in the image's
//! `.meta` sidecar beside the model settings, read by the renderer's upload
//! (`render::gpu::textures`), and written by the image inspector and
//! `Assets.SetTextureImportSettings` through [`set_texture`].

use serde::{Deserialize, Serialize};
use std::path::Path;

use super::{load, meta_path, save};
use crate::asset::ImportError;

/// How UVs outside 0–1 read the texture (Unity's `TextureWrapMode`; wgpu has no
/// `MirrorOnce`, so rusty leaves it out).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WrapMode {
    /// Tile the texture.
    #[default]
    Repeat,
    /// Stretch the edge texels.
    Clamp,
    /// Tile, flipping every other copy.
    Mirror,
}

/// How texels are blended when sampled (Unity's `FilterMode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FilterMode {
    /// The nearest texel: crisp, pixelated.
    Point,
    /// Blend the four nearest texels within a mip; snap between mips.
    Bilinear,
    /// Bilinear, blended between mips too, and anisotropic. rusty's default since
    /// every texture got a mip chain (#1322).
    #[default]
    Trilinear,
}

/// One texture's import settings. `#[serde(default)]` so a sparse sidecar loads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextureSettings {
    pub wrap_mode: WrapMode,
    pub filter_mode: FilterMode,
    /// Upload a full mip chain (Unity's *Generate Mip Maps*, on by default).
    pub generate_mipmaps: bool,
}

impl Default for TextureSettings {
    fn default() -> Self {
        Self {
            wrap_mode: WrapMode::default(),
            filter_mode: FilterMode::default(),
            generate_mipmaps: true,
        }
    }
}

impl WrapMode {
    pub const ALL: [Self; 3] = [Self::Repeat, Self::Clamp, Self::Mirror];

    /// The name the API and the sidecar use.
    pub fn name(self) -> &'static str {
        match self {
            Self::Repeat => "Repeat",
            Self::Clamp => "Clamp",
            Self::Mirror => "Mirror",
        }
    }

    /// The mode named `name`, refusing an unknown one with the list of known ones.
    pub fn parse(name: &str) -> Result<Self, String> {
        let found = Self::ALL.into_iter().find(|m| m.name() == name);
        found.ok_or_else(|| unknown("wrap mode", name, Self::ALL.map(Self::name)))
    }
}

impl FilterMode {
    pub const ALL: [Self; 3] = [Self::Point, Self::Bilinear, Self::Trilinear];

    /// The name the API and the sidecar use.
    pub fn name(self) -> &'static str {
        match self {
            Self::Point => "Point",
            Self::Bilinear => "Bilinear",
            Self::Trilinear => "Trilinear",
        }
    }

    /// The mode named `name`, refusing an unknown one with the list of known ones.
    pub fn parse(name: &str) -> Result<Self, String> {
        let found = Self::ALL.into_iter().find(|m| m.name() == name);
        found.ok_or_else(|| unknown("filter mode", name, Self::ALL.map(Self::name)))
    }
}

fn unknown(what: &str, name: &str, known: [&str; 3]) -> String {
    format!(
        "unknown {what} {name:?}; expected one of {}",
        known.join(", ")
    )
}

/// The import settings of the image at `source`: its sidecar's, or the defaults
/// when it has none or the sidecar does not parse.
pub fn texture(source: &Path) -> TextureSettings {
    load(source).map(|s| s.texture).unwrap_or_default()
}

/// Set the image at `source`'s import settings, keeping the rest of its sidecar.
/// The one setter the image inspector and `Assets.SetTextureImportSettings` share.
/// The write is noted in `core::fs::written`, so the renderer drops the uploaded
/// texture and re-uploads it with the new settings on the next frame.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_through_the_sidecar_and_a_missing_one_is_the_default() {
        let dir = crate::test_temp::dir();
        let png = dir.join("brick.png");
        assert_eq!(texture(&png), TextureSettings::default());
        let set = TextureSettings {
            wrap_mode: WrapMode::Mirror,
            filter_mode: FilterMode::Point,
            generate_mipmaps: false,
        };
        set_texture(&png, set).unwrap();
        assert_eq!(texture(&png), set);
        assert!(load(&png).unwrap().colliders.is_empty(), "the rest kept");
    }

    #[test]
    fn names_parse_back_and_an_unknown_one_lists_the_known() {
        for m in WrapMode::ALL {
            assert_eq!(WrapMode::parse(m.name()), Ok(m));
        }
        for m in FilterMode::ALL {
            assert_eq!(FilterMode::parse(m.name()), Ok(m));
        }
        let err = FilterMode::parse("Linear").unwrap_err();
        assert!(
            err.contains("\"Linear\"") && err.contains("Trilinear"),
            "{err}"
        );
    }
}
