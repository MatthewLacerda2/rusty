use serde::{Deserialize, Serialize};

/// A grey colour (`r = g = b`, white by default) tints nothing; only the offset then acts, as
/// Unity's trackball.
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LiftGammaGain {
    pub lift: ColorWheel,
    pub gamma: ColorWheel,
    pub gain: ColorWheel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wheel {
    Lift,
    Gamma,
    Gain,
}

impl Wheel {
    pub const ALL: [Wheel; 3] = [Wheel::Lift, Wheel::Gamma, Wheel::Gain];

    /// The one spelling, for the inspector and `Graphics.{Get,Set}<name>Wheel`.
    pub fn name(self) -> &'static str {
        match self {
            Wheel::Lift => "Lift",
            Wheel::Gamma => "Gamma",
            Wheel::Gain => "Gain",
        }
    }
}

impl LiftGammaGain {
    pub fn wheel(&self, which: Wheel) -> &ColorWheel {
        match which {
            Wheel::Lift => &self.lift,
            Wheel::Gamma => &self.gamma,
            Wheel::Gain => &self.gain,
        }
    }

    pub fn wheel_mut(&mut self, which: Wheel) -> &mut ColorWheel {
        match which {
            Wheel::Lift => &mut self.lift,
            Wheel::Gamma => &mut self.gamma,
            Wheel::Gain => &mut self.gain,
        }
    }
}
