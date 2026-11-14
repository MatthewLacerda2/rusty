//! crates/render/src/scene/stack_blend_gpu_tests.rs — a viewmodel between two ticks
//! (#1877): every camera entity is drawn blended like the meshes it draws, so a gun
//! parented to a stacked `DepthOnly` camera stays put on screen mid-blend, whether
//! that camera moves or the main camera it hangs from does.

use glam::Vec3;

use super::stack_gpu_tests::stack;
use crate::motion::gpu_fixture::{eye, renderer, view, H, W};
use crate::{readback, Renderer};
use rusty_scene::{game_camera_from_scene, Scene};

/// The entity named `name`.
fn named(scene: &Scene, name: &str) -> u32 {
    let ids = scene.world.ids().iter().copied();
    let mut found = ids.filter(|&id| scene.world.name(id).is_some_and(|n| *n == name));
    found.next().unwrap()
}

/// One frame through the scene's own main view, as the frontends pass it: the mean
/// column and the count of the gun's green pixels.
fn gun_on_screen(r: &mut Renderer, scene: &Scene) -> (f32, usize) {
    let mut v = view(r);
    let out = v.color_target_view().unwrap();
    r.render(
        &mut v,
        scene,
        &game_camera_from_scene(&eye(0.0), scene),
        &out,
        false,
    );
    let px = readback::read_texture_rgba8(&r.device, &r.queue, v.color_target().unwrap(), W, H);
    let green = |i: usize| px[i * 4 + 1] > px[i * 4].saturating_add(40).max(px[i * 4 + 2] + 40);
    let cols: Vec<u32> = (0..(W * H) as usize)
        .filter(|&i| green(i))
        .map(|i| i as u32 % W)
        .collect();
    let mean = cols.iter().sum::<u32>() as f32 / cols.len().max(1) as f32;
    (mean, cols.len())
}

/// The gun's place on screen half a tick after `moved` stepped 0.4 m right, and
/// with no blend: equal, though the gun itself is drawn 0.2 m back.
fn held_on_screen(r: &mut Renderer, moved: &str) {
    let (mut scene, _) = stack(true);
    let world = named(&scene, "Camera");
    scene.world.camera_mut(world).unwrap().taa_active = false;
    let gun_camera = named(&scene, "GunCamera");
    scene.set_parent(gun_camera, Some(world)).unwrap();
    let id = named(&scene, moved);
    r.draw_poses.record(&scene);
    scene.world.transform_mut(id).unwrap().position += Vec3::new(0.4, 0.0, 0.0);
    r.draw_poses.record(&scene);
    r.draw_poses.set_alpha(0.5);
    let blended = gun_on_screen(r, &scene);
    r.draw_poses.clear();
    let live = gun_on_screen(r, &scene);
    assert!(live.1 > 10, "{moved}: the gun is on screen");
    assert!(
        (blended.0 - live.0).abs() < 0.5,
        "{moved}: the gun stays on its camera: {blended:?} against {live:?}"
    );
}

#[test]
fn gpu_a_viewmodel_stays_on_its_camera_between_ticks() {
    let Some(mut r) = renderer() else {
        return;
    };
    held_on_screen(&mut r, "GunCamera"); // the stacked camera moves
    held_on_screen(&mut r, "Camera"); // the main camera it hangs from moves
}
