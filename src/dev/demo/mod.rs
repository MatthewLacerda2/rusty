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
//! - **Checked, not promised.** A digest of the world every two seconds
//!   (`digest`), compared on replay: the first that differs is reported.
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
    world.set_tap(Some(Box::new(Recorder::new(destination))));
}

/// What a dev build does on boot: record into the project's [`DEMOS_DIR`].
pub fn record_project(world: &mut GameWorld) {
    record(world, Destination::Folder(DEMOS_DIR.into()));
}

/// Replay the demo at `demo_path` headless on the open project, writing a
/// scenario run's results into `out_dir`. With `scenario_path` that scenario
/// drives the run (its steps advance the replay); without, the whole demo plays.
/// `size` is the run's render size, as `play --size`.
pub fn replay(
    demo_path: &Path,
    scenario_path: Option<&Path>,
    out_dir: &Path,
    size: Option<(u32, u32)>,
) -> Result<RunReport, String> {
    let demo = Demo::read(demo_path)?;
    let header = demo.header.clone();
    let mut harness = Harness::in_user_workspace(out_dir, &header.scene, header.settings)?;
    if let Some((width, height)) = size {
        harness.set_size(width, height);
    }
    let ticks = demo.ticks;
    prepare(&mut harness.world.borrow_mut(), demo)?;
    match scenario_path {
        Some(path) => scenario::run_file(harness, path),
        None => scenario::run_code(harness, &format!("Harness.Step({ticks})"), "=demo"),
    }
}

/// Write a replay's verdict into the run's results, when the run is one: a
/// divergence fails it, and each taint is logged.
pub fn conclude(harness: &mut Harness) {
    let verdict = {
        let mut world = harness.world.borrow_mut();
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
    match &verdict.diverged {
        Some(divergence) => harness.expect(false, format!("the demo replays: {divergence}")),
        None if verdict.checked > 0 => harness.expect(true, "the demo replays bit for bit".into()),
        None => {}
    }
}
