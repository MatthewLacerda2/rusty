use glam::Vec3;

use super::gpu_tests::{scaled, upscales};
use crate::motion::gpu_fixture::{cube, eye, pixel, project, renderer, scene, H, W};
use crate::postfx::effects::taa::gpu::fixture::{diff, frame, settle};
use crate::Renderer;
use rusty_scene::{Camera, Scene};

fn blurry(blur: bool) -> Scene {
    let (mut scene, cam) = scene(blur);
    let mut c = scene.world.camera_mut(cam).unwrap();
    (
        c.taa_active,
        c.motion_blur_intensity,
        c.motion_blur_max_length,
    ) = (true, 1.0, 0.2);
    drop(c);
    scene
}

fn moved(r: &mut Renderer, cam: &Camera, blur: bool) -> (Vec<u8>, bool, bool) {
    let mut scene = blurry(blur);
    let id = cube(&mut scene, Vec3::new(-0.5, 0.0, -5.0));
    let mut v = scaled(r, 0.5);
    settle(r, &mut v, &scene, cam, 8);
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.5, 0.0, -5.0);
    let after = frame(r, &mut v, &scene, cam);
    (after, upscales(&v), v.post_fx.holds_blur())
}

#[test]
fn gpu_motion_blur_after_taau_streaks_the_length_it_does_at_native() {
    let Some(mut r) = renderer() else { return };
    let cam = eye(0.0);
    let (blurred, upscaled, blurs) = moved(&mut r, &cam, true);
    let (sharp, _, _) = moved(&mut r, &cam, false);
    assert!(upscaled && blurs, "TAA upscaled the view and the blur ran");

    let (from, to) = (Vec3::new(-0.5, 0.0, -4.5), Vec3::new(0.5, 0.0, -4.5));
    let ndc = project(&cam, to).x - project(&cam, from).x;
    let half = 0.5 * (ndc * 0.5 * W as f32).min(0.2 * H as f32);
    eprintln!("[taau] motion blur: half a streak is {half:.1} output pixels");
    assert!(
        half >= 4.0,
        "the fixture streaks far enough to measure: {half}"
    );
    let reach = half.floor() as i32 - 1;
    for (edge, out) in [(1.0, 1), (0.0, -1)] {
        let (x, y) = pixel(project(&cam, Vec3::new(edge, 0.0, -4.5)));
        let at = |i: i32| ((x as i32 + out * i) as u32, y);
        let near = (1..=reach)
            .map(|i| diff(&blurred, &sharp, at(i)))
            .min()
            .unwrap();
        eprintln!("[taau] within {reach} px of x = {x}: the blur moved each by {near} or more");
        assert!(near >= 4, "the blur stops short at x = {x}: {near}");
    }
    let (x, y) = pixel(project(&cam, Vec3::new(1.0, 0.0, -4.5)));
    let far = half.ceil() as u32 + 3;
    let d = (far..far + 3)
        .map(|i| diff(&blurred, &sharp, (x + i, y)))
        .max();
    assert!(d.unwrap() <= 1, "the blur reaches past its streak: {d:?}");
}
