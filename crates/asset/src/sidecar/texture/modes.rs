use serde::{Deserialize, Serialize};

/// Unity's `TextureWrapMode`; wgpu has no `MirrorOnce`, so rusty leaves it out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WrapMode {
    #[default]
    Repeat,
    Clamp,
    Mirror,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FilterMode {
    Point,
    Bilinear,
    /// Bilinear, blended between mips too, and anisotropic: rusty's default.
    #[default]
    Trilinear,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Compression {
    None,
    #[default]
    Normal,
    HighQuality,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextureType {
    /// Four channels: BC7, mips averaged in linear light (sRGB colour, or raw where its slot holds data).
    #[default]
    Default,
    /// A tangent-space normal map: BC5 (x and y; the shader rebuilds z), linear.
    NormalMap,
    /// One channel (roughness, metallic, a mask), read from red: BC4, linear.
    SingleChannel,
}

impl WrapMode {
    pub const ALL: [Self; 3] = [Self::Repeat, Self::Clamp, Self::Mirror];

    pub fn name(self) -> &'static str {
        match self {
            Self::Repeat => "Repeat",
            Self::Clamp => "Clamp",
            Self::Mirror => "Mirror",
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        let found = Self::ALL.into_iter().find(|m| m.name() == name);
        found.ok_or_else(|| unknown("wrap mode", name, Self::ALL.map(Self::name)))
    }
}

impl FilterMode {
    pub const ALL: [Self; 3] = [Self::Point, Self::Bilinear, Self::Trilinear];

    pub fn name(self) -> &'static str {
        match self {
            Self::Point => "Point",
            Self::Bilinear => "Bilinear",
            Self::Trilinear => "Trilinear",
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        let found = Self::ALL.into_iter().find(|m| m.name() == name);
        found.ok_or_else(|| unknown("filter mode", name, Self::ALL.map(Self::name)))
    }
}

impl Compression {
    pub const ALL: [Self; 3] = [Self::None, Self::Normal, Self::HighQuality];

    pub fn name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Normal => "Normal",
            Self::HighQuality => "HighQuality",
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        let found = Self::ALL.into_iter().find(|m| m.name() == name);
        found.ok_or_else(|| unknown("compression", name, Self::ALL.map(Self::name)))
    }
}

impl TextureType {
    pub const ALL: [Self; 3] = [Self::Default, Self::NormalMap, Self::SingleChannel];

    pub fn name(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::NormalMap => "NormalMap",
            Self::SingleChannel => "SingleChannel",
        }
    }

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
