//! `frame_ms` times the tick a shipped, windowed game runs (#2342): from after its
//! play transition to its end, so it holds the tick's work outside any system, never
//! the entry into Play; a system run in every stage (`flush_bounds`) is one sample a
//! tick; a demo's taps and an inline load's preparation are timed beside it, not in
//! it. Every test drives the dev layer's clock (`verbs::drive`), so a charge is exact.

use rusty_app::{GameWorld, PlayTransition, SceneLoader, TapPoint, TickTap};
use rusty_core::frame_stats::FrameStats;
use rusty_edit::load::prepare::PreparedScene;

use super::stats_load_tests::world_with_level;
use super::verbs::{advance, drive};

/// What a fixture charges the driven clock, in ms.
const NAP: u64 = 30;
const NAP_MS: f64 = NAP as f64;

/// A loader whose job, once started, is finished at once and whose `take` costs
/// [`NAP`]: work done there when `inline`, as a headless host's loader does, else
/// a worker's result handed back, tick work outside the schedule and any system.
struct Loader {
    inline: bool,
    job: Option<u32>,
}

impl SceneLoader for Loader {
    fn start(&mut self, op: u32, _path: String) {
        self.job = Some(op);
    }
    fn finished(&mut self, op: u32) -> bool {
        self.job == Some(op)
    }
    fn take(&mut self, _op: u32) -> Result<PreparedScene, String> {
        self.job = None;
        advance(NAP);
        Err("a fixture prepares nothing".into())
    }
    fn cancel(&mut self) {}
    fn prepares_inline(&self) -> bool {
        self.inline
    }
}

/// A tap costing [`NAP`] as Play is entered (work before the span) and at the end
/// of every third tick, as a demo recorder digests the world every 120th.
#[derive(Default)]
struct Recorder(u32);

impl TickTap for Recorder {
    fn at(&mut self, point: TapPoint, _world: &mut GameWorld) {
        match point {
            TapPoint::Transition(PlayTransition::Entered) => advance(NAP),
            TapPoint::AfterTick => {
                self.0 += 1;
                if self.0.is_multiple_of(3) {
                    advance(NAP);
                }
            }
            _ => {}
        }
    }
}

/// `ticks` play ticks on a fresh world, `setup` run before Play, `playing` after
/// its first tick (which enters Play).
fn play(
    world: &str,
    ticks: u32,
    setup: impl FnOnce(&mut GameWorld),
    playing: impl FnOnce(&mut GameWorld, &str),
) -> FrameStats {
    drive();
    let (mut gw, stats, level) = world_with_level(world);
    setup(&mut gw);
    gw.set_playing(true);
    gw.tick(1.0 / 60.0);
    playing(&mut gw, &level);
    for _ in 1..ticks {
        gw.tick(1.0 / 60.0);
    }
    let stats = stats.borrow().clone();
    stats
}

#[test]
fn a_budget_across_play_entry_and_a_recorders_digest_reads_the_game_tick() {
    let tap = |gw: &mut GameWorld| gw.set_tap(Some(Box::new(Recorder::default())));
    let stats = play("recorded_budget", 6, tap, |_, _| {});
    let frame = stats.get("frame_ms").expect("ticks were timed");
    assert_eq!((frame.samples, frame.max), (6, 0.0), "no entry, no taps");
    assert!(stats.check_budget("frame_ms", 1.0).is_ok());
    let taps = stats.get("taps_ms").expect("an installed tap is timed");
    assert_eq!((taps.samples, taps.max), (6, NAP_MS), "beside the tick");
    let systems: Vec<_> = stats.tick_systems.iter().map(|(n, _)| *n).collect();
    assert!(
        !systems.contains(&"taps"),
        "the taps are no system: {systems:?}"
    );
}

#[test]
fn an_inline_async_load_is_beside_the_tick_and_a_workers_publish_in_it() {
    let load = |inline: bool| {
        let loader =
            |gw: &mut GameWorld| gw.set_scene_loader(Box::new(Loader { inline, job: None }));
        play(&format!("async_{inline}"), 3, loader, |gw, level| {
            gw.scene()
                .borrow_mut()
                .loads
                .request_async(level.to_string());
        })
    };
    let inline = load(true);
    assert_eq!(inline.get("frame_ms").map(|s| s.max), Some(0.0));
    assert!(
        inline.check_budget("frame_ms", 1.0).is_ok(),
        "the take is no tick's"
    );
    let loads = inline.get("loads_ms").map(|s| (s.samples, s.last));
    assert_eq!(loads, Some((1, NAP_MS)), "timed beside the tick it ran in");
    let worker = load(false);
    assert!(
        worker.get("loads_ms").is_none(),
        "a worker's publish is no part"
    );
    let frame = worker.get("frame_ms").map(|s| (s.samples, s.max));
    assert_eq!(
        frame,
        Some((3, NAP_MS)),
        "a worker's hand-back is the tick's"
    );
    assert_eq!(
        worker.systems["flush_bounds"].samples, 3,
        "one sample a tick"
    );
}

#[test]
fn an_edit_tick_is_no_frame_and_without_a_tap_there_is_no_taps_row() {
    drive();
    let (mut gw, stats, _) = world_with_level("no_tap");
    gw.tick(1.0 / 60.0);
    assert_eq!(stats.borrow().frames, 0, "an edit tick is no frame");
    gw.set_playing(true);
    gw.tick(1.0 / 60.0);
    let stats = stats.borrow();
    assert_eq!(stats.frames, 1);
    assert!(stats.get("taps_ms").is_none(), "{:?}", stats.metrics.keys());
}
