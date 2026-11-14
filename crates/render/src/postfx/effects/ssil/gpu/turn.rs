//! The accumulate rejects a surface's history when the surface turns in place
//! (#1750): its distance from the eye holds, but its normal no longer does.

use glam::{Quat, Vec3};

use super::fixture::{block, draw, eye, on, pixel_at, renderer, scene, view, LAMP, W};

/// The panel's centre: beside the lamp's pool, facing the camera.
const PANEL: Vec3 = Vec3::new(1.0, 0.8, -4.4);

/// Whether, after six frames with the panel at `from` degrees of yaw and one at
/// `to`, the panel's centre took no history: its result is this frame's blur.
fn starts_over(from: f32, to: f32) -> bool {
    let Some(mut r) = renderer() else {
        return from != to;
    };
    let (mut scene, _) = scene(on(1.0), LAMP);
    let panel = block(
        &mut scene,
        "panel",
        (PANEL, Vec3::new(0.8, 1.2, 0.05)),
        [0.9; 3],
    );
    let turn = |scene: &mut rusty_scene::Scene, yaw: f32| {
        scene.world.transform_mut(panel).unwrap().rotation =
            Quat::from_rotation_y(yaw.to_radians());
    };
    let (mut v, cam) = (view(&r), eye(0.0));
    turn(&mut scene, from);
    for _ in 0..6 {
        draw(&mut r, &mut v, &scene, &cam);
    }
    turn(&mut scene, to);
    draw(&mut r, &mut v, &scene, &cam);
    let (blurred, result, width) = v.post_fx.ssil_accumulate(&r.device, &r.queue);
    let (x, y) = pixel_at(PANEL, 0.0);
    let scale = W / width;
    let i = (y / scale * width + x / scale) as usize;
    eprintln!(
        "[ssil] the panel {from}° → {to}°: blur {}, result {}",
        blurred[i], result[i]
    );
    assert!(blurred[i].max_element() > 0.0, "the panel takes a bounce");
    result[i] == blurred[i]
}

#[test]
fn gpu_a_surface_turning_in_place_starts_its_history_over() {
    // 40 degrees: past the rejection's ~25, and the panel's centre stays put.
    assert!(starts_over(20.0, -20.0), "a turned panel takes no history");
}

#[test]
fn gpu_a_still_surface_keeps_its_history() {
    assert!(
        !starts_over(20.0, 20.0),
        "the control: a still panel blends its history in"
    );
}
