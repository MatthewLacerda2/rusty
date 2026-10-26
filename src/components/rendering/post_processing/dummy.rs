//! A test-only post effect (#1467): the worked example of adding one. It registers
//! in every layer through its own files and one row per registry, and draws nothing.

use serde::{Deserialize, Serialize};

/// The dummy's settings: a switch and one knob.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dummy {
    pub active: bool,
    /// `0..=1`.
    pub strength: f32,
}
