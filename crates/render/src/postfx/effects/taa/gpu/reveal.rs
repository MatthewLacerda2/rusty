use glam::Vec3;

use super::fixture::{eye, pixel, project, renderer, settle, taa_scene, view};
use super::wall::{grey_crate, luma, noisy_wall};
use crate::motion::gpu_fixture::W;
use rusty_scene::{Camera, Scene};

const AFTER: u32 = 2;
/// Measured on lavapipe: 2.25.
const RESIDUAL: f32 = 3.0;

fn revealed(cam: &Camera) -> Vec<usize> {
    let (x0, y0) = pixel(project(cam, Vec3::new(-1.0, 0.4, -4.0)));
    let (x1, y1) = pixel(project(cam, Vec3::new(-0.2, -0.4, -4.0)));
    let rows = y0 + 1..y1;
    rows.flat_map(|y| (x0 + 1..x1).map(move |x| (y * W + x) as usize))
        .collect()
}

fn crate_scene(x: f32) -> (Scene, u32) {
    let (mut scene, _) = taa_scene(true, true);
    noisy_wall(&mut scene, -7.0);
    let id = grey_crate(&mut scene, Vec3::new(x, 0.0, -4.5));
    (scene, id)
}

#[test]
fn gpu_a_crate_sliding_off_a_same_grey_wall_leaves_no_trace() {
    let Some(mut r) = renderer() else { return };
    let cam = eye(0.0);
    let (mut scene, id) = crate_scene(-0.6);
    let mut v = view(&r);
    let held = settle(&mut r, &mut v, &scene, &cam, 12);
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.6, 0.0, -4.5);
    let after = settle(&mut r, &mut v, &scene, &cam, AFTER);
    let (gone, _) = crate_scene(0.6);
    let mut fresh = view(&r);
    let reference = settle(&mut r, &mut fresh, &gone, &cam, 12 + AFTER);
    let (a, b, h) = (luma(&after), luma(&reference), luma(&held));
    let patch = revealed(&cam);
    let mean =
        |f: &dyn Fn(usize) -> f32| patch.iter().map(|&i| f(i)).sum::<f32>() / patch.len() as f32;
    assert!(
        mean(&|i| (h[i] - b[i]).abs()) > 4.0,
        "the crate was not there"
    );
    let residual = mean(&|i| (a[i] - b[i]).abs());
    eprintln!("[taa] revealed wall's residual after {AFTER} frames: {residual:.2}");
    assert!(
        residual <= RESIDUAL,
        "the crate smeared over the wall: {residual:.2}"
    );
}
