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
//! A play-mode scene load is timed the same way, step by step, by an installed
//! [`LoadProbe`] (#2123): the load hands it each step to run and tells it when the
//! load finished, and nothing the probe measures reaches the sim.
//!
//! No probe is installed by default, and a ship build never installs one.

use super::game::GameWorld;
use super::resources::Resources;
use super::stage::Stage;
use super::world::World;
use crate::scripting::ConsoleLogs;

/// Observes the schedule. Must run `system` exactly once per call — the schedule's
/// behaviour is unchanged by a probe, only watched.
pub trait SystemProbe {
    /// Run `system`, the system named `name` in `stage`.
    fn run(&mut self, stage: Stage, name: &'static str, system: &mut dyn FnMut());

    /// A per-frame schedule pass finished. Read-only: counters are read, never written.
    fn end_frame(&mut self, world: &World, res: &Resources);
}

/// Observes a play-mode scene load (`app::scene_load`), step by step. Must run
/// `step` exactly once per call — the load is unchanged by a probe, only watched.
pub trait LoadProbe {
    /// Run `step`, the load step named `name` (`read`, `meshes`, `physics`, …).
    fn step(&mut self, name: &'static str, step: &mut dyn FnMut());

    /// The load the steps since the last call belong to finished: `loaded` names
    /// the scene swapped in, or is `None` when the load failed and nothing was.
    fn finish(&mut self, loaded: Option<&str>, console: &mut ConsoleLogs);
}

/// Run `step` under `probe` as the load step `name`, or plainly without one.
pub(super) fn timed<T>(
    probe: &mut Option<Box<dyn LoadProbe>>,
    name: &'static str,
    step: impl FnOnce() -> T,
) -> T {
    let Some(probe) = probe.as_mut() else {
        return step();
    };
    let (mut step, mut out) = (Some(step), None);
    // A probe that calls the step twice runs it once; the second call does nothing.
    probe.step(name, &mut || {
        if let Some(run) = step.take() {
            out = Some(run());
        }
    });
    match (out, step) {
        (Some(out), _) => out,
        // A probe that broke its contract and skipped the step: run it here, once.
        (None, Some(run)) => run(),
        // Never: the step is only ever taken to fill `out`, which nothing empties.
        (None, None) => unreachable!("a step that ran left its result"),
    }
}

impl GameWorld {
    /// Install (or with `None`, remove) the schedule's probe.
    pub fn set_probe(&mut self, probe: Option<Box<dyn SystemProbe>>) {
        self.resources.probe = probe;
    }

    /// Install (or with `None`, remove) the scene-load probe.
    pub fn set_load_probe(&mut self, probe: Option<Box<dyn LoadProbe>>) {
        self.resources.load_probe = probe;
    }
}
