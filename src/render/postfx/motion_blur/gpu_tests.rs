//! Per-object motion blur on lavapipe (#1436): a moving cube blurs past its
//! silhouette, a still frame is untouched, and a viewmodel stays sharp while the
//! world blurs under a turning camera.

use glam::{Quat, Vec3};

use crate::render::motion::gpu_fixture::{cube, eye, pixel, project, renderer, scene, view};
use crate::render::motion::gpu_fixture::{H, W};
use crate::render::{readback, RenderView, Renderer};
use crate::scene::{Camera, ClearFlags, Scene};

/// Render `scene` through `cam` into `view` and read the finished frame (RGBA8).
fn frame(r: &mut Renderer, view: &mut RenderView, scene: &Scene, cam: &Camera) -> Vec<u8> {
    let out = view.color_target_view().unwrap();
    r.render(view, scene, cam, &out, false);
    readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), W, H)
}

/// The largest per-channel difference between two frames at pixel `(x, y)`.
fn diff(a: &[u8], b: &[u8], (x, y): (u32, u32)) -> u8 {
    let i = ((y * W + x) * 4) as usize;
    (0..3).map(|c| a[i + c].abs_diff(b[i + c])).max().unwrap()
}

/// A scene whose camera blurs (`blur`) with the longest streaks it allows: the
/// shutter open the whole frame, the clamp at its widest (9.6 px of 48).
fn streaky(blur: bool) -> (Scene, u32) {
    let (mut scene, cam) = scene(blur);
    let mut c = scene.world.camera_mut(cam).unwrap();
    c.motion_blur_intensity = 1.0;
    c.motion_blur_max_length = 0.2;
    drop(c);
    (scene, cam)
}

#[test]
fn gpu_a_moving_cube_blurs_past_its_silhouette() {
    let Some(mut r) = renderer() else { return };
    let cam = eye(0.0);
    let (mut moving, _) = streaky(true);
    let id = cube(&mut moving, Vec3::new(-0.5, 0.0, -5.0));
    let mut blurred_view = view(&r);
    frame(&mut r, &mut blurred_view, &moving, &cam);
    let to = Vec3::new(0.5, 0.0, -5.0);
    moving.world.transform_mut(id).unwrap().position = to;
    let blurred = frame(&mut r, &mut blurred_view, &moving, &cam);
    assert!(blurred_view.post_fx.blur.is_some(), "the blur ran");

    let (mut still, _) = streaky(false);
    cube(&mut still, to);
    let mut sharp_view = view(&r);
    let sharp = frame(&mut r, &mut sharp_view, &still, &cam);

    // Two pixels past each side of the front face, on the sky: the cube smears
    // onto it both ways.
    for (edge, out) in [(1.0, 2.0), (0.0, -2.0)] {
        let (x, y) = pixel(project(&cam, Vec3::new(edge, 0.0, -4.5)));
        let past = ((x as f32 + out) as u32, y);
        let d = diff(&blurred, &sharp, past);
        eprintln!("[motion_blur] {past:?} past the silhouette differs by {d}");
        assert!(
            d >= 4,
            "{past:?} is untouched: the blur stops at the silhouette"
        );
    }
    let far = (2, 2);
    assert!(diff(&blurred, &sharp, far) <= 1, "the far sky did not move");
}

#[test]
fn gpu_a_still_frame_is_identical_to_motion_blur_off() {
    let Some(mut r) = renderer() else { return };
    let cam = eye(0.0);
    let (mut on, _) = streaky(true);
    cube(&mut on, Vec3::new(0.0, 0.0, -5.0));
    let mut on_view = view(&r);
    frame(&mut r, &mut on_view, &on, &cam);
    let blurred = frame(&mut r, &mut on_view, &on, &cam);
    assert!(on_view.post_fx.blur.is_some(), "the blur ran");

    let (mut off, _) = streaky(false);
    cube(&mut off, Vec3::new(0.0, 0.0, -5.0));
    let mut off_view = view(&r);
    let sharp = frame(&mut r, &mut off_view, &off, &cam);
    assert!(off_view.post_fx.blur.is_none(), "no blur, no targets");
    assert!(blurred == sharp, "a still frame must be pixel-identical");
}

/// The world camera's scene with a cube at (2, 0, -6), and a `DepthOnly` overlay
/// camera holding a gun cube 3 m ahead. Returns it and the overlay camera.
fn viewmodel(blur: bool) -> (Scene, u32) {
    let (mut scene, _) = streaky(blur);
    cube(&mut scene, Vec3::new(2.0, 0.0, -6.0));
    let overlay = scene.add_entity("ViewmodelCamera".to_string());
    let lens = crate::components::CameraComponent {
        render_order: 1,
        clear_flags: ClearFlags::DepthOnly,
        ..Default::default()
    };
    scene.world.set_camera(overlay, Some(lens));
    let gun = cube(&mut scene, Vec3::new(0.0, 0.0, -3.0));
    scene.set_parent(gun, Some(overlay)).unwrap();
    (scene, overlay)
}

/// Turn the eye and the overlay camera together, `degrees` from -Z.
fn turn(scene: &mut Scene, overlay: u32, degrees: f32) -> Camera {
    let yaw = Quat::from_rotation_y(-degrees.to_radians());
    scene.world.transform_mut(overlay).unwrap().rotation = yaw;
    eye(degrees)
}

#[test]
fn gpu_a_viewmodel_stays_sharp_while_the_world_blurs() {
    let Some(mut r) = renderer() else { return };
    let (mut scene, overlay) = viewmodel(true);
    let mut blurred_view = view(&r);
    let before = turn(&mut scene, overlay, 0.0);
    frame(&mut r, &mut blurred_view, &scene, &before);
    let after = turn(&mut scene, overlay, 8.0);
    let blurred = frame(&mut r, &mut blurred_view, &scene, &after);

    let (mut plain, overlay) = viewmodel(false);
    let after = turn(&mut plain, overlay, 8.0);
    let mut sharp_view = view(&r);
    let sharp = frame(&mut r, &mut sharp_view, &plain, &after);

    let (cx, cy) = (W / 2, H / 2);
    let gun = |(x, y): (u32, u32)| x.abs_diff(cx) <= 3 && y.abs_diff(cy) <= 3;
    let pixels = (0..H).flat_map(|y| (0..W).map(move |x| (x, y)));
    let (mut on_gun, mut world) = (0, 0);
    for p in pixels {
        let d = diff(&blurred, &sharp, p);
        if gun(p) {
            on_gun = on_gun.max(d);
        } else {
            world = world.max(d);
        }
    }
    eprintln!("[motion_blur] viewmodel: gun differs by {on_gun}, world by {world}");
    assert!(on_gun <= 2, "the gun smeared: {on_gun}");
    assert!(world >= 8, "the world did not blur: {world}");
}
