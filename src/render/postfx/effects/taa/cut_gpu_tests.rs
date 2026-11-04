//! A cut on lavapipe (#1702): the first frame after a scene load is byte for byte a
//! new view's first, with no ghost of the last scene. Which frames are cuts (Play,
//! Stop, a camera switch, a reset) is `motion::cut_tests`'s, adapter-free.

use glam::Vec3;

use super::gpu_fixture::{cube, eye, frame, fresh_frame, renderer, settle, taa_scene, view};
use crate::render::motion::gpu_fixture::{H, W};

#[test]
fn gpu_a_scene_load_leaves_no_ghost_of_the_last_scene() {
    let Some(mut r) = renderer() else { return };
    let cam = eye(0.0);
    // A white cube on the left, settled; then another scene (another id: a cut, as
    // Play, Stop or a camera switch are) with its cube on the right, so the old
    // history and the new frame disagree on both halves.
    let (mut before, _) = taa_scene(true, true);
    cube(&mut before, Vec3::new(-0.9, 0.0, -4.0));
    let mut v = view(&r);
    settle(&mut r, &mut v, &before, &cam, 12);
    let (mut after, _) = taa_scene(true, true);
    cube(&mut after, Vec3::new(0.9, 0.0, -4.0));
    let first = frame(&mut r, &mut v, &after, &cam);
    let fresh = fresh_frame(&mut r, &after, &cam);
    let differing = (0..(W * H) as usize)
        .filter(|&i| first[i * 4..i * 4 + 4] != fresh[i * 4..i * 4 + 4])
        .count();
    assert_eq!(
        differing, 0,
        "the first frame after the cut is not a new view's first"
    );
}
