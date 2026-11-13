//! The named choices of a texture's import settings, each spelled as Unity spells
//! it: the API, the sidecar and the inspector use [`name`](WrapMode::name), and
//! an unknown name is refused with the list of known ones.

use serde::{Deserialize, Serialize};

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

/// How the texture is block-compressed on the GPU (Unity's *Compression*,
/// #1574). The format follows its [`TextureType`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Compression {
    /// Uncompressed RGBA8: four bytes a texel.
    None,
    /// The [`TextureType`]'s block format. Unity's default.
    #[default]
    Normal,
    /// The same formats, BC7 encoded slower and closer to the source.
    HighQuality,
}

/// What the texels hold (Unity's *Texture Type*, `TextureImporterType`, #1574):
/// it decides the block format and how the mips average, ahead of any draw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextureType {
    /// Colour or packed data, four channels: BC7, its mips averaged in linear
    /// light. Sampled as sRGB colour, or raw where its slot holds data.
    #[default]
    Default,
    /// A tangent-space normal map: BC5 (x and y; the shader rebuilds z), linear.
    NormalMap,
    /// One channel (roughness, metallic, a mask), read from red: BC4, linear.
    SingleChannel,
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

impl Compression {
    pub const ALL: [Self; 3] = [Self::None, Self::Normal, Self::HighQuality];

    /// The name the API and the sidecar use.
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Normal => "Normal",
            Self::HighQuality => "HighQuality",
        }
    }

    /// The compression named `name`, refusing an unknown one with the list of known ones.
    pub fn parse(name: &str) -> Result<Self, String> {
        let found = Self::ALL.into_iter().find(|m| m.name() == name);
        found.ok_or_else(|| unknown("compression", name, Self::ALL.map(Self::name)))
    }
}

impl TextureType {
    pub const ALL: [Self; 3] = [Self::Default, Self::NormalMap, Self::SingleChannel];

    /// The name the API and the sidecar use.
    pub fn name(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::NormalMap => "NormalMap",
            Self::SingleChannel => "SingleChannel",
        }
    }

    /// The type named `name`, refusing an unknown one with the list of known ones.
    pub fn parse(name: &str) -> Result<Self, String> {
        let found = Self::ALL.into_iter().find(|m| m.name() == name);
        found.ok_or_else(|| unknown("texture type", name, Self::ALL.map(Self::name)))
    }
}

fn unknown(what: &str, name: &str, known: [&str; 3]) -> String {
    format!(
        "unknown {what} {name:?}; expected one of {}",
        known.join(", ")
    )
}
