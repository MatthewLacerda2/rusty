//! Screen-space indirect lighting (#1750), Godot 4's SSIL (HDRP's SSGI in its cheap
//! form): one bounce of the frame's realtime light off nearby on-screen surfaces,
//! added to the indirect diffuse. A red wall a point light hits tints the white floor
//! in front of it. Off by default. The quality sets its sample count and working
//! resolution (`render::postfx::effects::ssil`).

use serde::{Deserialize, Serialize};

use rusty_core::settings::quality::EffectQuality;

/// Whether SSIL runs, its look, and its cost.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ssil {
    pub active: bool,
    /// How strongly the bounce lights a surface, `0..=4`: `1` is the gathered light
    /// as it is, above it exaggerated, `0` none.
    pub intensity: f32,
    /// World-space reach of the gather, `0.1..=10` metres: how far from a surface
    /// another lit surface still bounces light onto it.
    pub radius: f32,
    /// Which sample count and resolution run: a level of the active quality preset,
    /// or `samples` and `resolution_divisor` here when `Custom`.
    pub quality: EffectQuality,
    /// `Custom`: hemisphere samples per gather texel, `1..=32`.
    pub samples: u32,
    /// `Custom`: the gather runs at the view's size divided by this, `1..=4`.
    pub resolution_divisor: u32,
}

impl Ssil {
    /// The strongest intensity a write accepts.
    pub const MAX_INTENSITY: f32 = 4.0;
    /// The shortest and longest radius a write accepts.
    pub const MIN_RADIUS: f32 = 0.1;
    pub const MAX_RADIUS: f32 = 10.0;
    /// The most samples a write accepts.
    pub const MAX_SAMPLES: u32 = 32;
}

impl Default for Ssil {
    /// Off; when turned on, plain strength over two metres at the preset's `Medium`
    /// level (`Custom` starts at 8 samples at half resolution, Godot's Medium).
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
