//! The colour-grading knobs of a post-processing volume (#1440): white balance and
//! Lift / Gamma / Gain, Unity URP's overrides of those names. The renderer bakes
//! them, with contrast, saturation, hue shift and the tonemapper, into the view's
//! grading LUT.

use serde::{Deserialize, Serialize};

/// White balance (Unity's *White Balance* override): shifts the image's white point.
/// Both are `-100..=100`, neutral `0`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WhiteBalance {
    /// Positive warms (towards yellow), negative cools (towards blue).
    pub temperature: f32,
    /// Positive tints towards magenta, negative towards green.
    pub tint: f32,
}

impl WhiteBalance {
    /// How far either knob reaches from neutral.
    pub const RANGE: f32 = 100.0;

    /// Both knobs at neutral: the bake skips the white-point transform.
    pub fn is_neutral(&self) -> bool {
        self.temperature == 0.0 && self.tint == 0.0
    }
}

/// One trackball of Lift / Gamma / Gain: a colour whose hue tints that range of the
/// image, and an offset that brightens (`> 0`) or darkens (`< 0`) it. A grey colour
/// (`r = g = b`, white by default) tints nothing; only the offset then acts, which
/// is Unity's trackball.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorWheel {
    /// Linear RGB, each `0..=1`.
    pub color: [f32; 3],
    /// `-1..=1`, neutral `0`.
    pub offset: f32,
}

impl Default for ColorWheel {
    fn default() -> Self {
        Self {
            color: [1.0; 3],
            offset: 0.0,
        }
    }
}

/// Lift / Gamma / Gain (Unity's override of that name): lift moves the blacks, gamma
/// the midtones, gain the whites. Each wheel is neutral at its default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LiftGammaGain {
    pub lift: ColorWheel,
    pub gamma: ColorWheel,
    pub gain: ColorWheel,
}

/// Which wheel of [`LiftGammaGain`]: the one place a wheel is named, for the ops,
/// the inspector and the `Graphics.*` functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wheel {
    Lift,
    Gamma,
    Gain,
}

impl Wheel {
    pub const ALL: [Wheel; 3] = [Wheel::Lift, Wheel::Gamma, Wheel::Gain];

    /// Its name, as the inspector and the `Graphics.{Get,Set}<name>Wheel` functions
    /// spell it.
    pub fn name(self) -> &'static str {
        match self {
            Wheel::Lift => "Lift",
            Wheel::Gamma => "Gamma",
            Wheel::Gain => "Gain",
        }
    }
}

impl LiftGammaGain {
    /// The wheel `which`.
    pub fn wheel(&self, which: Wheel) -> &ColorWheel {
        match which {
            Wheel::Lift => &self.lift,
            Wheel::Gamma => &self.gamma,
            Wheel::Gain => &self.gain,
        }
    }

    /// The wheel `which`, mutably.
    pub fn wheel_mut(&mut self, which: Wheel) -> &mut ColorWheel {
        match which {
            Wheel::Lift => &mut self.lift,
            Wheel::Gamma => &mut self.gamma,
            Wheel::Gain => &mut self.gain,
        }
    }
}
