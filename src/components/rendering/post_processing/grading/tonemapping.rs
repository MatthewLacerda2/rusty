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
        /// Khronos PBR Neutral (#1701) — the default: colours below 0.76 pass 1:1
        /// with their hue and saturation, highlights roll off to white along a
        /// line that keeps the hue, so a warm lamp stays warm.
        #[default]
        Neutral = "Neutral",
        /// AgX (Blender 4, Godot 4.4) — a filmic log curve in a slightly inset
        /// gamut: saturated highlights desaturate towards white without skewing.
        AgX = "AgX",
        /// Narkowicz's fit of the ACES RRT + sRGB ODT, at the original curve's
        /// exposure (#1701) — rich contrast, but per channel, so saturated
        /// highlights skew in hue (orange goes yellow).
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
            Tonemap::Neutral => 3,
            Tonemap::AgX => 4,
        }
    }
}

/// The tonemapper and the display-gamma tweak, both baked into the grading LUT.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tonemapping {
    /// The operator; Neutral by default.
    pub mode: Tonemap,
    /// Display-gamma *tweak* around the neutral `1.0` (#415): the final image is
    /// `c^(1/gamma)` in linear space, then the sRGB render target does the real
    /// linear→display encode. `> 1` brightens midtones, `< 1` darkens them.
    pub display_gamma: f32,
}

impl Default for Tonemapping {
    fn default() -> Self {
        Self {
            mode: Tonemap::Neutral,
            display_gamma: 1.0,
        }
    }
}
