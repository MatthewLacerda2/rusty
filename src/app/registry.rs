//! src/app/registry.rs — App: the registry modules wire their systems into.
//!
//! `App` is the single thing modules see at startup. Each feature module exposes
//! `register(&mut App)` and pushes its systems into the stage they belong to; the
//! `App` owns the resulting [`Schedule`]. There is no plugin trait and no event
//! bus — registration is a direct method call, and cross-system signals stay
//! direct typed returns (CLAUDE.md).
//!
//! The default `App` is built by [`build`], which calls each module's `register`
//! in the exact order the old hand-wired `play.rs` ran them, so the per-frame
//! execution order is preserved bit-for-bit.
//!
//! Allowed deps: app::*.

use super::schedule::Schedule;
use super::stage::Stage;
use super::system::System;

/// The application registry. Holds the [`Schedule`] that modules register into.
#[derive(Default)]
pub struct App {
    schedule: Schedule,
}

impl App {
    /// A fresh `App` with an empty schedule.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `system` to run in `stage`. Order within a stage is the order of
    /// these calls (module registration order).
    pub fn add_system(&mut self, stage: Stage, system: System) -> &mut Self {
        self.schedule.add_system(stage, system);
        self
    }

    /// Consume the `App`, yielding the built schedule to drive the tick with.
    pub fn into_schedule(self) -> Schedule {
        self.schedule
    }
}

/// Build the default engine `App`: every built-in module registers its systems in
/// the canonical order. This is the single place the per-frame system order is
/// defined — modules self-register here instead of being hand-wired in `play.rs`.
pub fn build() -> App {
    let mut app = App::new();
    super::play::register(&mut app);
    app
}
