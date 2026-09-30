//! src/app/schedule.rs — Stage ordering
//!
//! Holds the ordered systems per stage and runs them in order. A `Schedule` is a
//! fixed table indexed by [`Stage`]; each slot is the list of systems registered
//! for that stage, kept in registration order. `run_startup` runs the one-shot
//! `Startup` stage; `run_frame` runs the four per-frame stages in
//! `FixedUpdate → Update → LateUpdate → Render` order (see [`Stage::FRAME_ORDER`]).
//!
//! Each system is stored with its function name, so an installed
//! [`SystemProbe`](super::SystemProbe) can attribute per-system cost (#433). With no
//! probe installed the frame runs the systems directly — the default, and the only
//! path a ship build takes.
//!
//! Allowed deps: app::probe, app::resources, app::stage, app::system, app::world.

use super::probe::SystemProbe;
use super::resources::Resources;
use super::stage::Stage;
use super::world::World;

/// A registered system, boxed so a named fn item keeps its type (and so its name)
/// up to registration.
type BoxedSystem = Box<dyn Fn(&mut World, &mut Resources)>;

/// One registered system: its short function name and the function itself.
struct Entry {
    name: &'static str,
    run: BoxedSystem,
}

/// The ordered set of systems per stage. Built once via [`super::App`]'s
/// `register` calls, then driven each tick against `(&mut World, &mut Resources)`.
#[derive(Default)]
pub struct Schedule {
    stages: [Vec<Entry>; Stage::COUNT],
}

impl Schedule {
    /// An empty schedule with no systems in any stage.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append `system` to `stage`. Execution order within a stage is the order of
    /// these calls — i.e. module registration order. The system's function name is
    /// kept for per-system stats.
    pub fn add_system<F>(&mut self, stage: Stage, system: F)
    where
        F: Fn(&mut World, &mut Resources) + 'static,
    {
        let name = short_name(std::any::type_name::<F>());
        self.stages[stage.index()].push(Entry {
            name,
            run: Box::new(system),
        });
    }

    /// The names of `stage`'s systems, in execution order.
    pub fn system_names(&self, stage: Stage) -> Vec<&'static str> {
        self.stages[stage.index()].iter().map(|e| e.name).collect()
    }

    /// Run every system registered for the one-shot `Startup` stage, in order.
    pub fn run_startup(&self, world: &mut World, res: &mut Resources) {
        self.run_stage(Stage::Startup, world, res, &mut None);
    }

    /// Run the per-frame stages in canonical order, each system in registration
    /// order. This is the real per-frame tick. An installed probe wraps every system
    /// and is told when the frame ends; it is moved out of `res` for the frame so the
    /// systems can take `&mut Resources`.
    pub fn run_frame(&self, world: &mut World, res: &mut Resources) {
        let mut probe = res.probe.take();
        for stage in Stage::FRAME_ORDER {
            self.run_stage(stage, world, res, &mut probe);
        }
        if let Some(probe) = probe.as_mut() {
            probe.end_frame(world, res);
        }
        res.probe = probe;
    }

    fn run_stage(
        &self,
        stage: Stage,
        world: &mut World,
        res: &mut Resources,
        probe: &mut Option<Box<dyn SystemProbe>>,
    ) {
        for entry in &self.stages[stage.index()] {
            match probe.as_mut() {
                Some(p) => p.run(stage, entry.name, &mut || (entry.run)(world, res)),
                None => (entry.run)(world, res),
            }
        }
    }
}

/// `rusty::app::play::tick_nav` → `tick_nav`. A non-item (a closure or a bare fn
/// pointer) has no useful path, so it reports as `system`.
fn short_name(type_name: &'static str) -> &'static str {
    if type_name.contains(['(', '{', '<']) {
        return "system";
    }
    type_name.rsplit("::").next().unwrap_or(type_name)
}
