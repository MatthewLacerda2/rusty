//! Unity HDRP's *Exposure* override: a fixed EV, or automatic exposure from the view's luminance
//! histogram.

use serde::{Deserialize, Serialize};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ExposureMode {
        #[default]
        Fixed = "Fixed",
        Automatic = "Automatic",
    }
}

/// The exposure the composite applies: the image is scaled by `2^ev`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoExposure {
    pub min_ev: f32,
    pub max_ev: f32,
    pub compensation: f32,
    /// EV per second the exposure rises at; eyes adapt to the dark more slowly than to light.
    pub speed_up: f32,
    pub speed_down: f32,
    /// `0..=100`.
    pub low_percent: f32,
    /// `low_percent..=100`.
    pub high_percent: f32,
}

impl AutoExposure {
    /// How far either limit reaches from 0 EV: the range the histogram meters.
    pub const EV_RANGE: f32 = 10.0;
    /// EV per second.
    pub const MAX_SPEED: f32 = 100.0;
}

impl Default for AutoExposure {
    /// Six stops either way, HDRP's adaptation speeds (light to dark 1, dark to light 3), 10%
    /// ignored at each end.
    fn default() -> Self {
        Self {
            min_ev: -6.0,
            max_ev: 6.0,
            compensation: 0.0,
            speed_up: 1.0,
            speed_down: 3.0,
            low_percent: 10.0,
            high_percent: 90.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Exposure {
    pub mode: ExposureMode,
    /// In EV; unused in `Automatic`.
    pub fixed_ev: f32,
    pub automatic: AutoExposure,
}
