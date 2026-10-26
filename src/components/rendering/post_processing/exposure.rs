//! How a post-processing volume exposes the image (#1444): a fixed EV, or automatic
//! exposure (eye adaptation) metered from the view's own luminance histogram, Unity
//! HDRP's *Exposure* override in its *Fixed* and *Automatic Histogram* modes.

use serde::{Deserialize, Serialize};

crate::core::named_enum! {
    /// Where the composite's pre-grade exposure comes from. Declared in the order the
    /// editor lists them (the default first); the names are Unity HDRP's.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub enum ExposureMode {
        /// The volume's `exposure` EV, as authored.
        #[default]
        Fixed = "Fixed",
        /// Metered every frame from the view's luminance histogram, and eased
        /// towards over game time ([`AutoExposure`]).
        Automatic = "Automatic",
    }
}

/// The knobs of [`ExposureMode::Automatic`]. Every EV here is the exposure the
/// composite applies (the image is scaled by `2^ev`), the unit of the volume's fixed
/// `exposure`: a dark room meters a positive EV, daylight a negative one.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoExposure {
    /// The darkest exposure adaptation may settle at: how far a bright scene can be
    /// stopped down.
    pub min_ev: f32,
    /// The brightest exposure adaptation may settle at: how far a dark scene can be
    /// opened up.
    pub max_ev: f32,
    /// Added to the metered EV before the limits: `> 0` keeps the image brighter
    /// than middle grey.
    pub compensation: f32,
    /// EV per second the exposure rises at (the image brightens: stepping into the
    /// dark). Eyes adapt to the dark more slowly than to light.
    pub speed_up: f32,
    /// EV per second the exposure falls at (the image darkens: stepping into light).
    pub speed_down: f32,
    /// The darkest percent of the histogram the meter ignores, `0..=100`.
    pub low_percent: f32,
    /// The percent of the histogram above which the meter ignores the brightest
    /// pixels, `low_percent..=100`.
    pub high_percent: f32,
}

impl AutoExposure {
    /// How far either limit reaches from `0` EV: the range the histogram meters.
    pub const EV_RANGE: f32 = 10.0;
    /// The fastest adaptation a write accepts, in EV per second.
    pub const MAX_SPEED: f32 = 100.0;
}

impl Default for AutoExposure {
    /// Six stops either way, no compensation, Unity HDRP's adaptation speeds (light
    /// to dark 1, dark to light 3) and the issue's histogram window, the darkest 10%
    /// and the brightest 10% ignored.
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
