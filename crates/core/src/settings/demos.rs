use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DemoSettings {
    pub keep: usize,
}

impl Default for DemoSettings {
    fn default() -> Self {
        Self { keep: 20 }
    }
}
