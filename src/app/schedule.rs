//! src/app/schedule.rs — Stage ordering
//!
//! Holds the ordered systems per stage and runs them in order. A `Schedule` is a
//! fixed table indexed by [`Stage`]; each slot is the list of systems registered
//! for that stage, kept in registration order. `run_startup` runs the one-shot
//! `Startup` stage; `run_frame` runs the four per-frame stages in
//! `FixedUpdate → Update → LateUpdate → Render` order (see [`Stage::FRAME_ORDER`]).
//!
//! Allowed deps: app::resources, app::stage, app::system, app::world.

use super::resources::Resources;
use super::stage::Stage;
use super::system::System;
use super::world::World;

/// The ordered set of systems per stage. Built once via [`super::App`]'s
/// `register` calls, then driven each tick against `(&mut World, &mut Resources)`.
#[derive(Default)]
pub struct Schedule {
    stages: [Vec<System>; Stage::COUNT],
}

impl Schedule {
    /// An empty schedule with no systems in any stage.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append `system` to `stage`. Execution order within a stage is the order of
    /// these calls — i.e. module registration order.
    pub fn add_system(&mut self, stage: Stage, system: System) {
        self.stages[stage.index()].push(system);
    }

    /// Run every system registered for the one-shot `Startup` stage, in order.
    pub fn run_startup(&self, world: &mut World, res: &mut Resources) {
        self.run_stage(Stage::Startup, world, res);
    }

    /// Run the per-frame stages in canonical order, each system in registration
    /// order. This is the real per-frame tick.
    pub fn run_frame(&self, world: &mut World, res: &mut Resources) {
        for stage in Stage::FRAME_ORDER {
            self.run_stage(stage, world, res);
        }
    }

    fn run_stage(&self, stage: Stage, world: &mut World, res: &mut Resources) {
        for system in &self.stages[stage.index()] {
            system(world, res);
        }
    }
}
