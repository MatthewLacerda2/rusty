//! Unity's `Application.backgroundLoadingPriority` (#2123): how long a frame may
//! spend on a `Scene.LoadAsync`'s main-thread work (its GPU warm-up) while the scene
//! before it still runs. A runtime setting, not saved: every run starts at the
//! default, as in Unity. It decides only how soon an operation reads ready, never
//! what the sim computes, so the host reads it, never a system.

use std::time::Duration;

use super::Application;

crate::named_enum! {
    /// Unity's `ThreadPriority`, as the loading budget reads it.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum LoadingPriority {
        Low = "Low",
        /// Unity's default.
        #[default]
        BelowNormal = "BelowNormal",
        Normal = "Normal",
        High = "High",
    }
}

impl LoadingPriority {
    /// The main-thread time a frame may spend on a load: Unity's 2, 4, 10 and 50 ms.
    pub fn budget(self) -> Duration {
        Duration::from_millis(match self {
            Self::Low => 2,
            Self::BelowNormal => 4,
            Self::Normal => 10,
            Self::High => 50,
        })
    }
}

impl Application {
    /// How long a frame may spend warming a background scene load.
    pub fn background_loading_priority(&self) -> LoadingPriority {
        self.loading_priority
    }

    /// Set it, for this run.
    pub fn set_background_loading_priority(&mut self, priority: LoadingPriority) {
        self.loading_priority = priority;
    }
}

#[cfg(test)]
#[path = "loading_tests.rs"]
mod tests;
