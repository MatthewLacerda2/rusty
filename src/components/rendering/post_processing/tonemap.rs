//! The tonemap operator: the final colour-correction step, and the one place its
//! names live (the editor's selector and `Graphics.*Tonemap` both read them, #997).

use serde::{Deserialize, Serialize};

crate::core::named_enum! {
    /// Tonemapping operator applied as the final color-correction step. Declared in
    /// the order the editor lists them (the default first); the names are Unity's
    /// (`TonemappingMode.ACES`, #997).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    pub fn to_index(self) -> u32 {
        match self {
            Tonemap::None => 0,
            Tonemap::Reinhard => 1,
            Tonemap::Aces => 2,
        }
    }
}
