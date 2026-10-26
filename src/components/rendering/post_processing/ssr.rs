//! Screen-space reflections, Unity HDRP's *Screen Space Reflection* override: run in
//! the composite on the High tier only.

use serde::{Deserialize, Serialize};

/// Whether SSR runs, and its quality preset name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ssr {
    pub active: bool,
    /// `"Low"`, `"Medium"`, `"High"` or `"Ultra"`.
    pub quality: String,
    pub temporal_upsampling: bool,
}

impl Default for Ssr {
    fn default() -> Self {
        Self {
            active: true,
            quality: "High".to_string(),
            temporal_upsampling: true,
        }
    }
}
