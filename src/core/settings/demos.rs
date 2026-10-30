//! src/core/settings/demos.rs — `DemoSettings`: how many play demos a project keeps (#1581).
//!
//! A dev build records every Play session to `saved/demos/` (`dev::demo`); the
//! project's `project.rusty` says, under `"demos"`, how many of the newest it keeps.

use serde::{Deserialize, Serialize};

/// The project's demo recording settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DemoSettings {
    /// How many demos `saved/demos/` keeps: the newest, the rest deleted as a new
    /// one starts. `0` records none.
    pub keep: usize,
}

impl Default for DemoSettings {
    fn default() -> Self {
        Self { keep: 20 }
    }
}
