//! stored-format: the `.demo` file a recorded playtest is saved as

mod calls;
mod delta;
mod digest;
mod files;
mod format;
mod forwards;
mod index;
mod read;
mod record;
mod replay;
mod script_ref;
mod sources;

use std::path::Path;

pub use calls::{journal_calls, plain_value};
pub use files::{demos_in, Destination, DEMOS_DIR};
pub use forwards::{Forward, FORWARDS};
pub use index::{find_in, Event, EventKind, IndexLine, Query, CAMERA_EVERY};
pub use read::{Demo, DemoTick, DemoTicks};
pub use record::Recorder;
pub use replay::{
    cut, cut_scenario, full_path, lend, prepare, seek, Divergence, Lent, Origin, Replayer, Verdict,
    NO_DEMO,
};
pub use script_ref::{note_call as note_script_call, note_set as note_script_set};

use crate::harness::Harness;
use crate::scenario::{self, RunReport};
use rusty_app::GameWorld;

/// A test hook: the integration tests compare a world with the digest a demo stores.
pub fn state_digest(world: &GameWorld) -> String {
    digest::of(world)
}

pub fn record(world: &mut GameWorld, destination: Destination) {
    world.script_manager().journal().borrow_mut().start();
    let recorder = Recorder::new(destination, world);
    world.set_tap(Some(Box::new(recorder)));
}

pub fn record_project(world: &mut GameWorld) {
    record(world, Destination::Folder(DEMOS_DIR.into()));
}

/// `run.size` is never read: the Game view is the demo's size, which the sim lays the UI on.
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
