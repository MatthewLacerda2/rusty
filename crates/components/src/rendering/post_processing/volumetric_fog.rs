use serde::{Deserialize, Serialize};

use rusty_core::settings::quality::EffectQuality;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VolumetricFog {
    pub active: bool,
    /// HDRP's *Fog Attenuation Distance*: metres at base height until all but `1/e` of light is
    /// gone, `1..=10000`.
    pub mean_free_path: f32,
    /// Each channel `0..=1`: white keeps every lamp's colour, black absorbs.
    pub albedo: [f32; 3],
    /// `-1..=1`, Henyey-Greenstein's g: `0` evenly, towards `1` onward, towards `-1` back.
    pub anisotropy: f32,
    pub base_height: f32,
    /// At least a centimetre above `base_height` when drawn.
    pub maximum_height: f32,
    /// HDRP's *Volumetric Fog Distance*, `4..=1000` metres.
    pub depth_extent: f32,
    /// `0..=1` (HDRP's *GI Dimmer*).
    pub global_light_probe_dimmer: f32,
    pub quality: EffectQuality,
    pub resolution: u32,
    /// `Custom`: depth slices from the camera to `depth_extent`, `16..=128`.
    pub slices: u32,
}

impl VolumetricFog {
    pub const MIN_MEAN_FREE_PATH: f32 = 1.0;
    pub const MAX_MEAN_FREE_PATH: f32 = 10000.0;
    pub const MAX_ANISOTROPY: f32 = 0.95;
    pub const MIN_DEPTH_EXTENT: f32 = 4.0;
    pub const MAX_DEPTH_EXTENT: f32 = 1000.0;
    pub const MAX_HEIGHT: f32 = 100_000.0;
    pub const MIN_RESOLUTION: u32 = 16;
    pub const MAX_RESOLUTION: u32 = 256;
    pub const MIN_SLICES: u32 = 16;
    pub const MAX_SLICES: u32 = 128;
}

impl Default for VolumetricFog {
    /// HDRP's defaults: a 400 m mean free path, white, isotropic, height 0 to `1/e` at 50 m.
    fn default() -> Self {
        Self {
            active: false,
            mean_free_path: 400.0,
            albedo: [1.0; 3],
            anisotropy: 0.0,
            base_height: 0.0,
            maximum_height: 50.0,
            depth_extent: 64.0,
            global_light_probe_dimmer: 1.0,
            quality: EffectQuality::Medium,
            resolution: 160,
            slices: 64,
        }
    }
}
