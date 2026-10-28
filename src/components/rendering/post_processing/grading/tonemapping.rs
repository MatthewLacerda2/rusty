//! Tonemapping, Unity URP's override of that name: the operator that brings HDR to
//! the display, and the one place its names live (the editor's selector and
//! `Graphics.*Tonemap` both read them, #997), with rusty's display-gamma tweak
//! (#415), applied last in the grade.

use serde::{Deserialize, Serialize};

crate::core::named_enum! {
    /// Tonemapping operator applied as the final color-correction step. Declared in
    /// the order the editor lists them (the default first); the names are Unity's
    /// (`TonemappingMode.ACES`, #997).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum Tonemap {
        /// Filmic ACES fit — the default, richer contrast in highlights.
        #[default]
        Aces = "ACES",
        /// Reinhard `c / (1 + c)` — cheap, soft rolloff.
        Reinhard = "Reinhard",
        /// No tonemap — clamp only. Mostly useful for debugging.
        None = "None",
    }
}

impl Tonemap {
    /// Stable index handed to the shader as a `u32`.
    pub const fn to_index(self) -> u32 {
        match self {
            Tonemap::None => 0,
            Tonemap::Reinhard => 1,
            Tonemap::Aces => 2,
        }
    }
}

/// The tonemapper and the display-gamma tweak, both baked into the grading LUT.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tonemapping {
    /// The operator; ACES by default.
    pub mode: Tonemap,
    /// Display-gamma *tweak* around the neutral `1.0` (#415): the final image is
    /// `c^(1/gamma)` in linear space, then the sRGB render target does the real
    /// linear→display encode. `> 1` brightens midtones, `< 1` darkens them.
    pub display_gamma: f32,
}

impl Default for Tonemapping {
    fn default() -> Self {
        Self {
            mode: Tonemap::Aces,
            display_gamma: 1.0,
        }
    }
}
