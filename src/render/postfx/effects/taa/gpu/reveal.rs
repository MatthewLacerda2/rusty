//! #1846 on lavapipe: a crate of the wall's own grey slides off a wall of noise,
//! and the wall it reveals shows its noise within two frames. The colour clamp
//! alone keeps the crate's flat grey (it sits inside the wall's neighbourhood
//! box); the depth test is what drops it.

use glam::Vec3;

use super::fixture::{eye, pixel, project, renderer, settle, taa_scene, view};
use super::wall::{grey_crate, luma, noisy_wall};
use crate::render::motion::gpu_fixture::W;
use crate::scene::{Camera, Scene};

/// Frames after the slide that are measured: the second after it.
const AFTER: u32 = 2;
/// The revealed patch's mean luma distance from a view that never saw the crate
/// there may be at most this. Measured on lavapipe: 2.25 with the depth test, 4.25
/// with URP's clamp alone (the depth test's thresholds pushed out of reach).
const RESIDUAL: f32 = 3.0;

/// The pixels the crate's front face covered before the slide, inset by one.
fn revealed(cam: &Camera) -> Vec<usize> {
    let (x0, y0) = pixel(project(cam, Vec3::new(-1.0, 0.4, -4.0)));
    let (x1, y1) = pixel(project(cam, Vec3::new(-0.2, -0.4, -4.0)));
    let rows = y0 + 1..y1;
    rows.flat_map(|y| (x0 + 1..x1).map(move |x| (y * W + x) as usize))
        .collect()
}

/// The fixture scene with the wall 7 m away and the crate at `x`.
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
    // The same frame count from scratch, the crate already moved: the same jitter.
    let (gone, _) = crate_scene(0.6);
    let mut fresh = view(&r);
    let reference = settle(&mut r, &mut fresh, &gone, &cam, 12 + AFTER);
    let (a, b, h) = (luma(&after), luma(&reference), luma(&held));
    let patch = revealed(&cam);
    let mean =
        |f: &dyn Fn(usize) -> f32| patch.iter().map(|&i| f(i)).sum::<f32>() / patch.len() as f32;
    // The crate drew there before, and its grey is not the wall's noise.
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
