//! TAAU's ghosts and cuts on lavapipe (#1702, part 3): a moving cube leaves no
//! trail at 0.67, and a render-scale change (what `Video.GetDisplayScale`'s monitor
//! move under `"match_display"`, `SetDefaultRenderScale`, `SetRenderScale` and a
//! demo's lend and give-back all come down to: `RenderView::resize` at a new scale)
//! is a cut, the next frame byte-equal to a fresh view's.

use glam::Vec3;

use super::gpu_tests::{fresh_at, scaled, upscales};
use crate::core::settings::render_scale::RenderScale;
use crate::render::motion::gpu_fixture::{H, W};
use crate::render::postfx::effects::taa::gpu_fixture::{
    cube, diff, eye, frame, pixel, project, renderer, settle, taa_scene,
};

/// How far off the settled sky where the cube was may be on the first frame after
/// the move and 4 frames on (lavapipe measures 15 and 4), with slack for another
/// driver's rounding: a trail fades, it never sticks.
const FIRST: u8 = 24;
const AFTER_4: u8 = 6;

#[test]
fn gpu_a_moving_cube_leaves_no_trail_when_upscaled() {
    let Some(mut r) = renderer() else { return };
    let cam = eye(0.0);
    let (mut scene, _) = taa_scene(true, true);
    let id = cube(&mut scene, Vec3::new(-0.6, 0.0, -5.0));
    let mut v = scaled(&r, 0.67);
    settle(&mut r, &mut v, &scene, &cam, 24);
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.6, 0.0, -5.0);
    let first = frame(&mut r, &mut v, &scene, &cam);
    let last = settle(&mut r, &mut v, &scene, &cam, 4);
    // The empty sky, settled through TAAU too, so only the trail differs.
    let (sky, _) = taa_scene(true, true);
    let mut e = scaled(&r, 0.67);
    let empty = settle(&mut r, &mut e, &sky, &cam, 24);
    let was = pixel(project(&cam, Vec3::new(-0.8, 0.0, -4.5)));
    let (d0, d4) = (diff(&first, &empty, was), diff(&last, &empty, was));
    eprintln!("[taau] where the cube was: {d0} off the sky the first frame, {d4} after 4");
    assert!(upscales(&v));
    assert!(d0 <= FIRST, "the first frame ghosts the cube: {d0}");
    assert!(d4 <= AFTER_4 && d4 <= d0, "the cube left a trail: {d4}");
}

#[test]
fn gpu_a_render_scale_change_leaves_no_ghost() {
    let Some(mut r) = renderer() else { return };
    let cam = eye(0.0);
    let (mut scene, _) = taa_scene(true, true);
    let id = cube(&mut scene, Vec3::new(-0.8, 0.0, -5.0));
    let mut v = scaled(&r, 0.5);
    settle(&mut r, &mut v, &scene, &cam, 24);
    // The scale changes and the cube moves: nothing of the 0.5 history may show.
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.8, 0.0, -5.0);
    v.set_render_scale(RenderScale::new(0.67));
    v.resize(&r.device, W, H);
    let after = frame(&mut r, &mut v, &scene, &cam);
    let fresh = fresh_at(&mut r, 0.67, &scene, &cam);
    assert!(upscales(&v), "still upscaling at 0.67");
    assert!(
        after == fresh,
        "the frame after a scale change is a fresh view's"
    );
    // And back to native: the plain TAA path, a fresh view's first frame too.
    cube(&mut scene, Vec3::new(0.0, 0.8, -5.0));
    v.set_render_scale(RenderScale::NATIVE);
    v.resize(&r.device, W, H);
    let native = frame(&mut r, &mut v, &scene, &cam);
    let fresh = fresh_at(&mut r, 1.0, &scene, &cam);
    assert!(
        !upscales(&v) && native == fresh,
        "native after a scale: a fresh view"
    );
}
