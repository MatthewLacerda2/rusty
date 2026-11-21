//! Godot 4's SSIL (HDRP's SSGI in its cheap form): one bounce of the frame's realtime light off
//! on-screen surfaces. Off by default.

use serde::{Deserialize, Serialize};

use rusty_core::settings::quality::EffectQuality;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ssil {
    pub active: bool,
    /// `0..=4`: `1` is the gathered light as it is, above it exaggerated.
    pub intensity: f32,
    /// `0.1..=10` metres.
    pub radius: f32,
    pub quality: EffectQuality,
    pub samples: u32,
    pub resolution_divisor: u32,
}

impl Ssil {
    pub const MAX_INTENSITY: f32 = 4.0;
    pub const MIN_RADIUS: f32 = 0.1;
    pub const MAX_RADIUS: f32 = 10.0;
    pub const MAX_SAMPLES: u32 = 32;
}

impl Default for Ssil {
    /// Plain strength over two metres at the preset's `Medium` level (`Custom` starts at 8 samples, half resolution).
    fn default() -> Self {
        Self {
            active: false,
            intensity: 1.0,
            radius: 2.0,
            quality: EffectQuality::Medium,
            samples: 8,
            resolution_divisor: 2,
        }
    }
}
