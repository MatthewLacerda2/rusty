use glam::Vec3;

use super::fixture::{cube, eye, frame, fresh_frame, renderer, settle, taa_scene, view};
use crate::motion::gpu_fixture::{H, W};

#[test]
fn gpu_a_scene_load_leaves_no_ghost_of_the_last_scene() {
    let Some(mut r) = renderer() else { return };
    let cam = eye(0.0);
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
