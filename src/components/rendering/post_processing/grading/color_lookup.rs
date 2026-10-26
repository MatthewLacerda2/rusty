//! Color Lookup (#1441), Unity's override of that name: a user LUT file (a `.cube`,
//! or a Unity strip `.png`) applied last, on the display colour.

use serde::{Deserialize, Serialize};

/// The LUT file and how much of it applies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorLookup {
    /// The LUT file, a project-relative path; none by default.
    pub lut: Option<String>,
    /// How much of [`lut`](Self::lut) applies, `0..=1`.
    pub contribution: f32,
}

impl Default for ColorLookup {
    fn default() -> Self {
        Self {
            lut: None,
            contribution: 1.0,
        }
    }
}
