//! What counts as a cut in the motion-vector history (#1435), adapter-free: Play and
//! Stop, and a camera swapped in at another's place in the stack.

use glam::{Mat4, Vec3};

use super::tests::one;
use super::{HistoryKey, MotionHistory};
use crate::scene::Scene;

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
