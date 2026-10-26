//! Stack order before depth (#1436): a `DepthOnly` gun camera's gun is in front
//! of the world even where the world is nearer to its own camera than the gun is to
//! the gun camera, the player standing against a wall.

use glam::Vec3;

use super::gpu_tests::{diff, frame, streaky};
use super::overlay_gpu_tests::{paint, VIEWMODEL};
use crate::components::CameraComponent;
use crate::render::motion::gpu_fixture::{cube, eye, renderer, view, H, W};
use crate::scene::{ClearFlags, Scene};

/// A blue wall whose face is 0.35 m from the world camera, filling the right of
/// the view, and a green gun 0.5–0.7 m ahead of a `DepthOnly` gun camera, over the
/// centre and the wall's left edge: the gun's right side borders the wall.
fn against_a_wall(blur: bool) -> Scene {
    let (mut scene, world) = streaky(blur);
    scene.world.camera_mut(world).unwrap().culling_mask = !(1 << VIEWMODEL);
    let wall = cube(&mut scene, Vec3::new(0.265, 0.0, -0.4));
    scene.world.transform_mut(wall).unwrap().scale = Vec3::new(0.47, 3.0, 0.1);
    paint(&mut scene, wall, "wall", [0.1, 0.1, 0.8]);
    let camera = scene.add_entity("GunCamera".to_string());
    let lens = CameraComponent {
        render_order: 1,
        clear_flags: ClearFlags::DepthOnly,
        culling_mask: 1 << VIEWMODEL,
        ..Default::default()
    };
    scene.world.set_camera(camera, Some(lens));
    let gun = cube(&mut scene, Vec3::new(0.0, 0.0, -0.6));
    scene.world.transform_mut(gun).unwrap().scale = Vec3::splat(0.2);
    paint(&mut scene, gun, "gun", [0.1, 0.8, 0.1]);
    scene.world.set_layer(gun, VIEWMODEL);
    scene.set_parent(gun, Some(camera)).unwrap();
    scene
}

#[test]
fn gpu_the_gun_stays_over_a_wall_nearer_to_its_own_camera() {
    let Some(mut r) = renderer() else { return };
    // The world camera turns 8 degrees; the gun camera holds still.
    let mut scene = against_a_wall(true);
    let mut v = view(&r);
    frame(&mut r, &mut v, &scene, &eye(0.0));
    let blurred = frame(&mut r, &mut v, &scene, &eye(8.0));
    assert!(v.post_fx.holds_blur(), "the blur ran");
    scene = against_a_wall(false);
    let mut v = view(&r);
    let sharp = frame(&mut r, &mut v, &scene, &eye(8.0));

    // The gun's inside, two pixels clear of its silhouette, which meets the wall's
    // left edge: the wall must not smear over it.
    let on_gun = gun_inside(&sharp)
        .map(|p| diff(&blurred, &sharp, p))
        .max()
        .expect("the gun is on screen");
    // The rows above the gun, where the wall's left edge meets the sky: it blurs.
    let above = (0..H / 4).flat_map(|y| (0..W).map(move |x| (x, y)));
    let wall = above.map(|p| diff(&blurred, &sharp, p)).max().unwrap();
    eprintln!("[motion_blur] against a wall: gun differs by {on_gun}, wall edge by {wall}");
    assert!(on_gun <= 2, "the wall smeared over the gun: {on_gun}");
    assert!(wall >= 8, "the wall did not blur: {wall}");
}

/// The pixels of `frame` whose every pixel within two, across and down, is the
/// green gun.
fn gun_inside(frame: &[u8]) -> impl Iterator<Item = (u32, u32)> + '_ {
    let green = move |x: i32, y: i32| {
        let inside = (0..W as i32).contains(&x) && (0..H as i32).contains(&y);
        let i = ((y * W as i32 + x) * 4) as usize;
        inside && frame[i + 1] > frame[i + 2].saturating_add(40)
    };
    let pixels = (0..H).flat_map(|y| (0..W).map(move |x| (x, y)));
    pixels.filter(move |&(x, y)| {
        let (x, y) = (x as i32, y as i32);
        (-2..=2).all(|d| green(x + d, y) && green(x, y + d))
    })
}
