//! What counts as a cut in the motion-vector history (#1435), adapter-free: Play and
//! Stop, and a camera swapped in at another's place in the stack; and the game time
//! the history hands the vertex stage (#1524).

use glam::{Mat4, Vec3};

use super::tests::one;
use super::{HistoryKey, MotionHistory};
use rusty_scene::Scene;

fn at(x: f32) -> Mat4 {
    Mat4::from_translation(Vec3::X * x)
}

#[test]
fn play_and_stop_are_cuts_though_the_scene_keeps_its_id() {
    let (mut scene, mut h) = (Scene::new(), MotionHistory::default());
    let mut frame = |scene: &Scene, cam: f32| {
        let prev = h.begin(HistoryKey::new(scene, Some(1)), &one(at(cam)));
        let had_pose = h.pose(7).is_some();
        h.record(7, at(cam), &[]);
        h.end();
        (prev, had_pose)
    };
    frame(&scene, 1.0);
    assert_eq!(frame(&scene, 2.0), (vec![at(1.0)], true), "an edit frame");
    scene.set_playing(true);
    assert_eq!(frame(&scene, 3.0), (vec![at(3.0)], false), "Play is a cut");
    assert_eq!(frame(&scene, 4.0), (vec![at(3.0)], true));
    // Stop restores the edit scene under the same `SceneId`: still a cut.
    scene.set_playing(false);
    assert_eq!(frame(&scene, 5.0), (vec![at(5.0)], false), "Stop is a cut");
}

#[test]
fn a_camera_swapped_in_at_the_same_place_has_no_last_frame() {
    let (k, mut h) = (
        super::tests::key(Scene::new().id(), Some(1)),
        MotionHistory::default(),
    );
    // World camera 1, viewmodel camera 2 on top.
    h.begin(k, &[(Some(1), at(1.0)), (Some(2), at(10.0))]);
    h.end();
    // A weapon switch: viewmodel camera 3 replaces 2 at the same place.
    let prev = h.begin(k, &[(Some(1), at(2.0)), (Some(3), at(30.0))]);
    assert_eq!(
        prev,
        [at(1.0), at(30.0)],
        "camera 3 never saw camera 2's matrix"
    );
    h.end();
    let prev = h.begin(k, &[(Some(1), at(3.0)), (Some(3), at(31.0))]);
    assert_eq!(prev, [at(2.0), at(30.0)]);
}

#[test]
fn game_time_is_last_frames_and_a_cut_forgets_it() {
    let (k, mut h) = (
        super::tests::key(Scene::new().id(), Some(1)),
        MotionHistory::default(),
    );
    let mut frame = |key, now: f32| {
        h.begin(key, &one(at(0.0)));
        let prev = h.game_time(now);
        h.end();
        prev
    };
    assert_eq!(frame(k, 2.5), 2.5, "no last frame: this frame's time");
    assert_eq!(frame(k, 3.25), 2.5);
    assert_eq!(frame(k, 4.0), 3.25);
    let cut = super::tests::key(Scene::new().id(), Some(1));
    assert_eq!(frame(cut, 7.0), 7.0, "a new scene is a cut");
    assert_eq!(frame(cut, 7.5), 7.0);
}

#[test]
fn reset_is_a_cut_under_the_same_key() {
    let (k, mut mv) = (
        super::tests::key(Scene::new().id(), Some(1)),
        super::MotionVectors::default(),
    );
    mv.history.begin(k, &one(at(1.0)));
    mv.history.record(7, at(1.0), &[]);
    mv.history.end();
    mv.reset();
    let prev = mv.history.begin(k, &one(at(2.0)));
    assert_eq!(prev, [at(2.0)], "the camera kept last frame's matrix");
    assert!(mv.history.pose(7).is_none(), "the entity kept its pose");
}

/// `cut()`, what TAA forgets its history on (#1702): the view's first frame, a
/// changed key (another scene, Play or Stop, another base camera, a camera's
/// `Camera.ResetHistory`) and a reset (a resize), and nothing else.
#[test]
fn cut_names_every_frame_with_no_last_frame() {
    let (scene, mut h) = (Scene::new(), MotionHistory::default());
    let cut = |h: &mut MotionHistory, key| {
        h.begin(key, &one(at(0.0)));
        h.end();
        h.cut()
    };
    let k = super::tests::key(scene.id(), Some(1));
    assert!(cut(&mut h, k), "the view's first frame");
    assert!(!cut(&mut h, k), "the next frame of the same stack");
    assert!(
        cut(&mut h, super::tests::key(Scene::new().id(), Some(1))),
        "a scene load"
    );
    let other = super::tests::key(scene.id(), Some(2));
    assert!(cut(&mut h, other), "a camera switch");
    assert!(!cut(&mut h, other));
    let play = HistoryKey {
        playing: true,
        ..other
    };
    assert!(cut(&mut h, play), "Play");
    let mut cuts = rusty_scene::camera::HistoryCuts::default();
    cuts.cut(Some(2));
    let reset = HistoryKey {
        cuts: cuts.of(Some(2)),
        ..play
    };
    assert!(cut(&mut h, reset), "Camera.ResetHistory (#2003)");
    assert!(!cut(&mut h, reset));
    h.clear();
    assert!(cut(&mut h, play), "a reset (a resize)");
    assert!(!cut(&mut h, play));
}
