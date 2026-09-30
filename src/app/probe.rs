//! src/app/probe.rs — the schedule's observation hook (#433).
//!
//! Frame stats need wall-clock timings, and the sim may not read a clock (the
//! determinism rule, `CLAUDE.md`). So the sim does not measure anything: it hands
//! each system to an installed [`SystemProbe`] to run, and the probe — implemented in
//! the dev layer, which is exempt — does the timing around the call. The probe gets
//! read-only access to the world when a frame ends (for counters) and writes into its
//! own stats store; nothing it produces is ever read back by a system, so the sim is
//! the same pure function of (seed, inputs, fixed dt) with or without one installed.
//!
//! No probe is installed by default, and a ship build never installs one.
//!
//! Allowed deps: app::*.

use super::game::GameWorld;
use super::resources::Resources;
use super::stage::Stage;
use super::world::World;

/// Observes the schedule. Must run `system` exactly once per call — the schedule's
/// behaviour is unchanged by a probe, only watched.
pub trait SystemProbe {
    /// Run `system`, the system named `name` in `stage`.
    fn run(&mut self, stage: Stage, name: &'static str, system: &mut dyn FnMut());

    /// A per-frame schedule pass finished. Read-only: counters are read, never written.
    fn end_frame(&mut self, world: &World, res: &Resources);
}

impl GameWorld {
    /// Install (or with `None`, remove) the schedule's probe.
    pub fn set_probe(&mut self, probe: Option<Box<dyn SystemProbe>>) {
        self.resources.probe = probe;
    }
}
