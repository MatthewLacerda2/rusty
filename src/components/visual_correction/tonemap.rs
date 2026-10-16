//! The tonemap operator: the final colour-correction step, and the one place its
//! names live (the editor's selector and `Graphics.*Tonemap` both read them, #997).

use serde::{Deserialize, Serialize};

/// Tonemapping operator applied as the final color-correction step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tonemap {
    /// No tonemap — clamp only. Mostly useful for debugging.
    None,
    /// Reinhard `c / (1 + c)` — cheap, soft rolloff.
    Reinhard,
    /// Filmic ACES fit — the default, richer contrast in highlights.
    #[default]
    Aces,
}

impl Tonemap {
    /// Every operator, in the order the editor lists them (the default first).
    pub const ALL: [Tonemap; 3] = [Tonemap::Aces, Tonemap::Reinhard, Tonemap::None];

    /// The operator's name, as the editor and the API spell it — Unity's
    /// (`TonemappingMode.ACES`, #997).
    pub fn name(self) -> &'static str {
        match self {
            Tonemap::None => "None",
            Tonemap::Reinhard => "Reinhard",
            Tonemap::Aces => "ACES",
        }
    }

    /// Parse an operator name, case-insensitively (so the pre-#997 `"Aces"` still
    /// parses); `None` for an unknown one.
    pub fn parse(name: &str) -> Option<Tonemap> {
        Self::ALL
            .into_iter()
            .find(|t| t.name().eq_ignore_ascii_case(name))
    }

    /// Stable index handed to the shader as a `u32`.
    pub fn to_index(self) -> u32 {
        match self {
            Tonemap::None => 0,
            Tonemap::Reinhard => 1,
            Tonemap::Aces => 2,
        }
    }
}
