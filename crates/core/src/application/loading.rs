//! Unity's `Application.backgroundLoadingPriority`. Not saved: every run starts at
//! the default. It never affects what the sim computes, so only the host reads it.

use std::time::Duration;

use super::Application;

crate::named_enum! {
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
    pub fn background_loading_priority(&self) -> LoadingPriority {
        self.loading_priority
    }

    pub fn set_background_loading_priority(&mut self, priority: LoadingPriority) {
        self.loading_priority = priority;
    }
}

#[cfg(test)]
#[path = "loading_tests.rs"]
mod tests;
