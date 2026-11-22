//! Unity URP's Depth of Field: *Gaussian* blurs the background between two distances; *Bokeh*
//! models a lens.

use serde::{Deserialize, Serialize};

use rusty_core::settings::quality::EffectQuality;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum DofMode {
        #[default]
        Off = "Off",
        Gaussian = "Gaussian",
        Bokeh = "Bokeh",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DepthOfField {
    pub mode: DofMode,
    /// Gaussian: metres from the camera where the blur starts.
    pub gaussian_start: f32,
    /// Gaussian: where the blur reaches its full radius; nearer than `gaussian_start` it is drawn
    /// at the start.
    pub gaussian_end: f32,
    pub gaussian_max_radius: f32,
    /// Bokeh: the distance in focus, in metres.
    pub focus_distance: f32,
    /// Bokeh: the f-stop, `1..=32`; lower blurs more.
    pub aperture: f32,
    /// Bokeh: focal length in millimetres, `1..=300`; longer blurs more.
    pub focal_length: f32,
    pub quality: EffectQuality,
    /// `Custom`: the view's size divided by this, `1..=4`.
    pub resolution_divisor: u32,
}

impl DepthOfField {
    pub const MIN_MAX_RADIUS: f32 = 0.5;
    pub const MAX_MAX_RADIUS: f32 = 1.5;
    pub const MIN_APERTURE: f32 = 1.0;
    pub const MAX_APERTURE: f32 = 32.0;
    pub const MIN_FOCAL_LENGTH: f32 = 1.0;
    pub const MAX_FOCAL_LENGTH: f32 = 300.0;
    pub const MIN_FOCUS_DISTANCE: f32 = 0.1;
    /// Metres: where `+inf` (focus at infinity) lands.
    pub const MAX_FOCUS_DISTANCE: f32 = 10_000.0;
}

impl Default for DepthOfField {
    /// Off, with URP's defaults, at the preset's `Medium` level (`Custom` starts at half
    /// resolution).
    fn default() -> Self {
        Self {
            mode: DofMode::Off,
            gaussian_start: 10.0,
            gaussian_end: 30.0,
            gaussian_max_radius: 1.0,
            focus_distance: 10.0,
            aperture: 5.6,
            focal_length: 50.0,
            quality: EffectQuality::Medium,
            resolution_divisor: 2,
        }
    }
}
