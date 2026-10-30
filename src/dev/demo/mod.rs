//! src/dev/demo/ — demos: every Play session recorded, replayed headless bit for
//! bit (#1581), DOOM's `-record` / `-playdemo`.
//!
//! The sim is a pure function of (seed, inputs, fixed dt), so a Play session is
//! kept whole by its start (scene, settings, save data, seed) and what reached it
//! tick by tick: the input each tick published, and every act of a host on the
//! world between ticks — a console or MCP line, a scenario's API call. A dev build
//! records each Play session (the editor's, `play`'s, the dev player's) into
//! `saved/demos/`, keeping the newest `demos.keep` (`project.rusty`, default 20);
//! `play --demo <file>` replays one headless, with a scenario on top if asked.
//!
//! - **One hook.** Recording and replay are [`TickTap`](crate::app::TickTap)s on
//!   `GameWorld::tick`, which every host drives: [`Recorder`] reads each tick's
//!   `InputFrame` right before it is published, [`Replayer`] writes it back there.
//! - **Host acts** go through the world's journal (`scripting::HostJournal`): the
//!   one REPL evaluator notes its lines, a scenario VM's API calls are noted by
//!   [`journal_calls`]. What cannot be carried as plain data (an Inspector edit,
//!   a script handle's calls) taints the demo from that tick, and the replay warns.
//! - **Checked, not promised.** A digest of the world every two seconds and one
//!   when Play ends (`digest`), compared on replay: the first that differs is
//!   reported.
//! - **Every tick's `dt`.** The editor and the dev player tick on wall-clock
//!   time; a demo keeps each tick's `dt` (only when it changes), and the replay
//!   runs on it.
//! - **Nothing of it in a ship build**: the whole module is `dev`.
//!
//! The file format is in `format`; the per-tick reader a search or a scrub reads
//! is [`Demo::ticks`].

mod calls;
mod delta;
mod digest;
mod files;
mod format;
mod read;
mod record;
mod replay;
mod sources;

use std::path::Path;

pub use calls::journal_calls;
pub use files::{demos_in, Destination, DEMOS_DIR};
pub use read::{Demo, DemoTick, DemoTicks};
pub use record::Recorder;
pub use replay::{prepare, Divergence, Replayer, Verdict};

use crate::app::GameWorld;
use crate::dev::harness::Harness;
use crate::dev::scenario::{self, RunReport};

/// Record every Play session of `world` to `destination` from now on.
pub fn record(world: &mut GameWorld, destination: Destination) {
    world.script_manager().journal().borrow_mut().start();
    let recorder = Recorder::new(destination, world);
    world.set_tap(Some(Box::new(recorder)));
}

/// What a dev build does on boot: record into the project's [`DEMOS_DIR`].
pub fn record_project(world: &mut GameWorld) {
    record(world, Destination::Folder(DEMOS_DIR.into()));
}

/// Replay the demo at `demo_path` headless on the open project, writing a
/// scenario run's results into `out_dir`. With `scenario_path` that scenario
/// drives the run (its steps advance the replay); without, the whole demo plays.
/// `run.record_audio` captures the replay's mix, as `play --record-audio`; the
/// Game view size is the demo's (the sim lays the UI out on it), so `run.size` is
/// not read.
pub fn replay(
    demo_path: &Path,
    scenario_path: Option<&Path>,
    out_dir: &Path,
    run: scenario::Run,
) -> Result<RunReport, String> {
    let demo = Demo::read(demo_path)?;
    let header = demo.header.clone();
    let mut harness = Harness::in_user_workspace(out_dir, &header.scene, header.settings)?;
    if let Some(path) = run.record_audio {
        harness.record_audio(Some(path))?;
    }
    let ticks = demo.ticks;
    let mut world = harness.world.borrow_mut();
    prepare(&mut world, demo)?;
    if let (Some(_), Some(replayer)) = (scenario_path, world.tap_mut::<Replayer>()) {
        replayer.driven_by_scenario();
    }
    drop(world);
    match scenario_path {
        Some(path) => scenario::run_file(harness, path),
        None => scenario::run_code(harness, &format!("Harness.Step({ticks})"), "=demo"),
    }
}

/// End a run's demo: a recording one's end line (its final digest) is written,
/// and a replay's verdict goes into the run's results: a divergence fails it,
/// and each taint is logged.
pub fn conclude(harness: &mut Harness) {
    let verdict = {
        let mut world = harness.world.borrow_mut();
        if let Some(mut tap) = world.take_tap() {
            let any: &mut dyn std::any::Any = tap.as_mut();
            if let Some(recorder) = any.downcast_mut::<Recorder>() {
                recorder.stop(&world);
            }
            world.set_tap(Some(tap));
        }
        world.tap_mut::<Replayer>().map(|r| r.verdict().clone())
    };
    let Some(verdict) = verdict else {
        return;
    };
    harness.log(format!(
        "demo: replayed {} of {} ticks, {} digests compared",
        verdict.ticks, verdict.of, verdict.checked
    ));
    for (t, what) in &verdict.taints {
        harness.log(format!("demo tainted from tick {t}: {what}"));
    }
    let short = verdict.ticks < verdict.of;
    match &verdict.diverged {
        Some(divergence) => harness.expect(false, format!("the demo replays: {divergence}")),
        // Its end digest never compared: only a scenario may stop it short.
        None if short && !verdict.scenario => harness.expect(
            false,
            format!(
                "the demo replays: stopped at tick {} of {}",
                verdict.ticks, verdict.of
            ),
        ),
        None if short => harness.expect(
            true,
            format!(
                "the demo replays bit for bit as far as the scenario stepped it (tick {} of {})",
                verdict.ticks, verdict.of
            ),
        ),
        None if verdict.checked > 0 => harness.expect(true, "the demo replays bit for bit".into()),
        None => {}
    }
}
