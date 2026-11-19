//! crates/app/src/probe.rs — the schedule's observation hook (#433).
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
//! load finished, and nothing the probe measures reaches the sim. Entering Play
//! is timed by a probe of the same kind installed apart ([`GameWorld::set_entry_probe`]):
//! the import refresh, the scripts' init, physics and the `Startup` stage, each a
//! step, then one `finish`, so a boot load (`make bench-scenes`) splits its first
//! tick without a second copy of the timing. A host times its own steps through
//! [`timed`] too.
//!
//! The probe also brackets each whole [`GameWorld::tick`] (`begin_tick`, `end_tick`),
//! so the tick's work outside any system (the UI textures, the camera and audio
//! sync) is in what it times, not only the systems' sum.
//!
//! No probe is installed by default, and a ship build never installs one.

use super::game::GameWorld;
use super::resources::Resources;
use super::stage::Stage;
use super::world::World;
use rusty_script::scripting::ConsoleLogs;

/// Observes the schedule. Must run `system` exactly once per call — the schedule's
/// behaviour is unchanged by a probe, only watched.
pub trait SystemProbe {
    /// Run `system`, the system named `name` in `stage`.
    fn run(&mut self, stage: Stage, name: &'static str, system: &mut dyn FnMut());

    /// A per-frame schedule pass finished. Read-only: counters are read, never written.
    fn end_frame(&mut self, world: &World, res: &Resources);

    /// A host's [`GameWorld::tick`] starts: what a probe times whole begins here.
    fn begin_tick(&mut self) {}

    /// That tick ended, after its schedule pass's [`end_frame`](Self::end_frame)
    /// when it ran one (in Play) and after the tick's own work around the schedule.
    fn end_tick(&mut self) {}
}

/// Observes a load step by step: a play-mode scene load (`app::scene_load`),
/// entering Play ([`GameWorld::set_entry_probe`]), or a host's own boot load run
/// through [`timed`] (`make bench-scenes`). Must run `step` exactly once per
/// call — the load is unchanged by a probe, only watched.
pub trait LoadProbe {
    /// Run `step`, the load step named `name` (`read`, `meshes`, `refresh`, …).
    fn step(&mut self, name: &'static str, step: &mut dyn FnMut());

    /// The load the steps since the last call belong to finished: `loaded` names
    /// the scene swapped in or Play entered, or is `None` when nothing was: the
    /// scene load failed, or the scripts' runtime failed to start, which ends
    /// Play's entry early.
    fn finish(&mut self, loaded: Option<&str>, console: &mut ConsoleLogs);
}

/// The installed [`LoadProbe`]s: a play-mode scene load's (the dev layer's load
/// timer, `GameWorld::set_load_probe`) and Play entry's (`GameWorld::set_entry_probe`).
#[derive(Default)]
pub struct LoadProbes {
    pub(super) scene: Option<Box<dyn LoadProbe>>,
    pub(super) entry: Option<Box<dyn LoadProbe>>,
}

/// Run `step` under `probe` as the load step `name`, or plainly without one: the
/// one way a step is timed, by a scene load, Play's entry and a host's boot load.
pub fn timed<T>(
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
    /// Advance the simulation by `dt` seconds; the play-state transition is returned
    /// for the platform (cursor grab). Every tick reads the UI's texture sizes after
    /// the transition (the schedule's in Play, the edit frame's outside), so no loop
    /// that ticks skips them, and ends by pushing the audio mix and advancing `dt`.
    /// An installed probe is told when the tick begins and ends, so it times it whole.
    pub fn tick(&mut self, dt: f32) -> super::PlayTransition {
        if let Some(p) = self.resources.probe.as_mut() {
            p.begin_tick();
        }
        let transition = self.run_tick(dt);
        if let Some(p) = self.resources.probe.as_mut() {
            p.end_tick();
        }
        transition
    }

    /// Install (or with `None`, remove) the schedule's probe.
    pub fn set_probe(&mut self, probe: Option<Box<dyn SystemProbe>>) {
        self.resources.probe = probe;
    }

    /// Install (or with `None`, remove) the scene-load probe.
    pub fn set_load_probe(&mut self, probe: Option<Box<dyn LoadProbe>>) {
        self.resources.load_probes.scene = probe;
    }

    /// Install (or with `None`, remove) the probe that times entering Play: its
    /// steps `refresh`, `scripts`, `physics` and `startup`, then a `finish` naming
    /// the scene Play entered (`None` when the scripts' runtime failed to start,
    /// which ends the entry early). Apart from the scene-load probe, so a Play
    /// entry never adds steps to a scene load's report.
    pub fn set_entry_probe(&mut self, probe: Option<Box<dyn LoadProbe>>) {
        self.resources.load_probes.entry = probe;
    }
}
