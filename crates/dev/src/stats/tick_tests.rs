//! `frame_ms` times the tick (#2342): from after its play transition to its end,
//! so it holds the tick's work outside any system, never the entry into Play; a
//! system run in every stage (`flush_bounds`) is one sample a tick; a demo's taps
//! are a row of their own.

use std::thread::sleep;
use std::time::Duration;

use rusty_app::{GameWorld, PlayTransition, SceneLoader, TapPoint, TickTap};
use rusty_core::frame_stats::FrameStats;
use rusty_edit::load::prepare::PreparedScene;

use super::stats_load_tests::world_with_level;

/// What the fixtures sleep: far above an empty scene's tick, so a margin is strict.
const NAP: Duration = Duration::from_millis(30);
const NAP_MS: f64 = 30.0;

/// A loader whose `cancel`, which every tick's head runs while no load is
/// pending, naps: tick work outside the schedule and outside any system.
struct SlowHead;

impl SceneLoader for SlowHead {
    fn start(&mut self, _op: u32, _path: String) {}
    fn finished(&mut self, _op: u32) -> bool {
        false
    }
    fn take(&mut self, _op: u32) -> Result<PreparedScene, String> {
        Err("no job".into())
    }
    fn cancel(&mut self) {
        sleep(NAP);
    }
}

/// A tap that naps as Play is entered: work in the transition, before the span.
struct SlowEntry;

impl TickTap for SlowEntry {
    fn at(&mut self, point: TapPoint, _world: &mut GameWorld) {
        if point == TapPoint::Transition(PlayTransition::Entered) {
            sleep(NAP);
        }
    }
}

/// `frame_ms` less the tick's system rows: what no system held.
fn other(stats: &FrameStats) -> f64 {
    let systems: f64 = stats.tick_systems.iter().map(|(_, ms)| ms).sum();
    stats.get("frame_ms").map_or(0.0, |s| s.last) - systems
}

#[test]
fn frame_ms_holds_the_ticks_work_outside_its_systems() {
    let (mut gw, stats, _) = world_with_level("whole_tick");
    gw.set_scene_loader(Box::new(SlowHead));
    gw.tick(1.0 / 60.0);
    assert_eq!(stats.borrow().frames, 0, "an edit tick is no frame");
    gw.set_playing(true);
    gw.tick(1.0 / 60.0);
    gw.tick(1.0 / 60.0);
    let stats = stats.borrow();
    assert_eq!(stats.get("frame_ms").map(|s| s.samples), Some(2));
    let other = other(&stats);
    assert!(
        other >= NAP_MS,
        "the head's nap is outside every system: {other}"
    );
    let flush = &stats.systems["flush_bounds"];
    assert_eq!(
        flush.samples, 2,
        "one sample a tick, summed over the stages"
    );
}

#[test]
fn a_budget_across_play_entry_reads_the_ticks_not_the_entry() {
    let (mut gw, stats, _) = world_with_level("entry_budget");
    gw.set_tap(Some(Box::new(SlowEntry)));
    gw.set_playing(true);
    for _ in 0..3 {
        gw.tick(1.0 / 60.0);
    }
    let stats = stats.borrow();
    assert_eq!(stats.get("frame_ms").map(|s| s.samples), Some(3));
    let budget = stats.check_budget("frame_ms", NAP_MS / 2.0);
    assert!(budget.is_ok(), "the entry's nap is no tick's: {budget:?}");
    let taps = stats.tick_systems.iter().any(|(n, _)| *n == "taps");
    assert!(taps, "an installed tap is a row: {:?}", stats.tick_systems);
}

#[test]
fn without_a_tap_there_is_no_taps_row() {
    let (mut gw, stats, _) = world_with_level("no_tap");
    gw.set_playing(true);
    gw.tick(1.0 / 60.0);
    let stats = stats.borrow();
    assert!(
        !stats.systems.contains_key("taps"),
        "{:?}",
        stats.systems.keys()
    );
}
