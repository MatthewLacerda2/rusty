//! A thin wire under TAAU on lavapipe (#1702, part 3; #1846): it stays
//! continuous at render scale 0.5, and at 0.33 no pixel of it reads as a reveal.
//! There the reveal test's 3×3 of output pixels would span a single scene pixel,
//! so it steps in scene pixels: a pixel the jitter misses the wire in this frame
//! still finds the sky beside it in the history, and does not flicker.

use glam::Vec3;

use super::gpu_tests::{fresh_at, scaled, FRAMES, SUPER};
use crate::motion::gpu_fixture::{H, W};
use crate::postfx::effects::taa::gpu::fixture::{
    big_view, cube, downsample, eye, frame, renderer, settle, taa_scene,
};
use crate::postfx::effects::taa::gpu::still::{differing, run, OFF};
use rusty_scene::Scene;

/// The wire's thickness: an output pixel, half a scene pixel at 0.5, a third of
/// one at 0.33.
const WIRE: f32 = 0.12;

/// The columns a wire across the frame covers, and how much the dimmest one of
/// them shows it: each column's largest difference from the empty sky.
fn faintest_column(wire: &[u8], sky: &[u8]) -> u8 {
    let px = |f: &[u8], x: u32, y: u32| {
        let i = ((y * W + x) * 4) as usize;
        [f[i], f[i + 1], f[i + 2]]
    };
    (8..W - 8)
        .map(|x| {
            (0..H)
                .map(|y| {
                    let (a, b) = (px(wire, x, y), px(sky, x, y));
                    (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap()
                })
                .max()
                .unwrap()
        })
        .min()
        .unwrap()
}

/// A wire `thick` metres across the fixture's view, 5 m out and slightly tilted.
fn wire(scene: &mut Scene, thick: f32) {
    let id = cube(scene, Vec3::new(0.0, 0.0, -5.0));
    let t = &mut scene.world.transform_mut(id).unwrap();
    t.scale = Vec3::new(8.0, thick, thick);
    t.rotation = glam::Quat::from_rotation_z(0.12);
}

/// The wire's faintest column at `scale`: bilinear, TAAU, and 8x native.
fn wire_at(scale: f32) -> Option<(u8, u8, u8)> {
    let mut r = renderer()?;
    let cam = eye(0.0);
    let wire_in = |scene: &mut Scene| wire(scene, WIRE);
    let (sky_scene, _) = taa_scene(false, false);
    let sky = fresh_at(&mut r, scale, &sky_scene, &cam);
    let (mut plain, _) = taa_scene(false, false);
    wire_in(&mut plain);
    let bilinear = faintest_column(&fresh_at(&mut r, scale, &plain, &cam), &sky);
    let (mut scene, _) = taa_scene(true, true);
    wire_in(&mut scene);
    let mut v = scaled(&r, scale);
    let taau = faintest_column(&settle(&mut r, &mut v, &scene, &cam, FRAMES), &sky);
    let mut big = big_view(&r, SUPER);
    let reference = downsample(&frame(&mut r, &mut big, &plain, &cam), SUPER);
    let bytes: Vec<u8> = reference
        .iter()
        .flat_map(|p| [p.x as u8, p.y as u8, p.z as u8, 255])
        .collect();
    let native = faintest_column(&bytes, &fresh_at(&mut r, 1.0, &sky_scene, &cam));
    eprintln!("[taau] the wire's faintest column at {scale}: bilinear {bilinear}, TAAU {taau}; 8x native {native}");
    Some((bilinear, taau, native))
}

#[test]
fn gpu_a_thin_wire_at_half_scale_stays_continuous() {
    let Some((bilinear, taau, native)) = wire_at(0.5) else {
        return;
    };
    // Lavapipe: bilinear 0 (a gap), TAAU 7, the reference 16.
    assert!(native >= 12, "the wire shows in the reference: {native}");
    assert!(
        4 * bilinear < native,
        "the wire must break up bilinearly: {bilinear}"
    );
    assert!(
        3 * taau >= native,
        "the wire has a gap under TAAU: {taau} of {native}"
    );
}

#[test]
fn gpu_no_pixel_of_a_wire_at_a_third_scale_reads_as_a_reveal() {
    let Some(mut r) = renderer() else { return };
    let still = |_: u32| eye(0.0);
    // An output pixel thick, and a scene pixel (three output pixels).
    for thick in [WIRE, 3.0 * WIRE] {
        let (mut scene, _) = taa_scene(true, true);
        wire(&mut scene, thick);
        let (mut a, mut b) = (scaled(&r, 0.33), scaled(&r, 0.33));
        let on = run(&mut r, &mut a, (&scene, &still), 32, Default::default());
        let off = run(&mut r, &mut b, (&scene, &still), 32, OFF);
        let n = differing(&on, &off);
        assert_eq!(n, 0, "a wire {thick} m thick: {n} bytes read as a reveal");
    }
}
