use serde::{Deserialize, Serialize};

/// Both are `-100..=100`, neutral `0`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WhiteBalance {
    pub temperature: f32,
    pub tint: f32,
}

impl WhiteBalance {
    pub const RANGE: f32 = 100.0;

    pub fn is_neutral(&self) -> bool {
        self.temperature == 0.0 && self.tint == 0.0
    }
}
