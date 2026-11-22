use serde::{Deserialize, Serialize};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum Tonemap {
        /// Khronos PBR Neutral, the default: colours below 0.76 pass 1:1, highlights roll off to
        /// white keeping hue.
        #[default]
        Neutral = "Neutral",
        /// AgX: per channel, so a saturated orange turns towards yellow as ACES does; pick Neutral
        /// to keep warm lamps orange.
        AgX = "AgX",
        /// Narkowicz's fit of the ACES RRT + sRGB ODT: per channel, so saturated highlights skew in
        /// hue.
        Aces = "ACES",
        Reinhard = "Reinhard",
        None = "None",
    }
}

impl Tonemap {
    /// The `case`s of `tonemapping.wgsl`'s operator switch: change the two together.
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tonemapping {
    pub mode: Tonemap,
    /// Display-gamma tweak around the neutral `1.0`: the final image is `c^(1/gamma)` in linear
    /// space. `> 1` brightens midtones.
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
