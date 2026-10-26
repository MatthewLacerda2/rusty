//! The exposure clock's snaps and steps, and the params' clamps, adapter-free.

use super::{Clock, ExposureParams, Step};
use crate::components::AutoExposure;
use crate::render::motion::HistoryKey;
use crate::scene::Scene;

fn key(scene: &Scene, playing: bool) -> HistoryKey {
    HistoryKey {
        playing,
        ..HistoryKey::new(scene, Some(1))
    }
}

const SNAP: Step = Step {
    dt: 0.0,
    snap: true,
    cut: true,
};

#[test]
fn play_eases_by_game_time_and_a_pause_holds() {
    let scene = Scene::new();
    let mut clock = Clock::default();
    assert_eq!(
        clock.step(key(&scene, true), false, 1.0),
        SNAP,
        "first frame"
    );
    let step = clock.step(key(&scene, true), false, 1.25);
    assert_eq!((step.dt, step.snap), (0.25, false));
    let paused = clock.step(key(&scene, true), false, 1.25);
    assert_eq!((paused.dt, paused.snap), (0.0, false), "time scale 0");
}

#[test]
fn edit_mode_cuts_and_time_running_back_snap() {
    let (scene, other) = (Scene::new(), Scene::new());
    let mut clock = Clock::default();
    clock.step(key(&scene, true), false, 2.0);
    assert_eq!(
        clock.step(key(&scene, true), false, 1.0),
        SNAP,
        "time ran back"
    );
    assert_eq!(
        clock.step(key(&other, true), false, 1.5),
        SNAP,
        "a scene load"
    );
    assert_eq!(clock.step(key(&other, false), false, 0.0), SNAP, "Stop");
    let held = clock.step(key(&other, false), false, 0.0);
    assert_eq!(
        (held.snap, held.cut),
        (true, false),
        "outside Play: a snap, no cut"
    );
    clock.step(key(&other, true), false, 3.0);
    clock.cut();
    assert_eq!(clock.step(key(&other, true), false, 3.5), SNAP, "a resize");
}

#[test]
fn swapping_the_editor_tabs_is_a_cut() {
    let scene = Scene::new();
    let mut clock = Clock::default();
    clock.step(key(&scene, true), false, 1.0);
    assert!(!clock.step(key(&scene, true), false, 1.1).cut);
    assert_eq!(
        clock.step(key(&scene, true), true, 1.2),
        SNAP,
        "Game → Scene"
    );
    assert_eq!(
        clock.step(key(&scene, true), false, 1.3),
        SNAP,
        "Scene → Game"
    );
}

#[test]
fn params_hold_a_loaded_file_to_the_ops_ranges() {
    let ae = AutoExposure {
        min_ev: 4.0,
        max_ev: -50.0,
        compensation: f32::NAN,
        speed_up: -2.0,
        speed_down: f32::INFINITY,
        low_percent: 95.0,
        high_percent: 20.0,
    };
    let p = ExposureParams::new(&ae, SNAP);
    assert_eq!(p.limits, [4.0, 4.0, 0.0, 0.0]);
    assert_eq!(p.adapt, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(p.window, [0.95, 0.95, 0.0, 0.0]);
}
