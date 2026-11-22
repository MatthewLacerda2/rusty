use glam::Vec3;

use super::gpu_tests::{diff, frame, streaky};
use super::overlay_gpu_tests::{paint, VIEWMODEL};
use crate::motion::gpu_fixture::{cube, eye, renderer, view, H, W};
use rusty_components::CameraComponent;
use rusty_scene::{ClearFlags, Scene};

pub(crate) fn against_a_wall(blur: bool) -> (Scene, u32) {
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
    (scene, world)
}

#[test]
fn gpu_the_gun_stays_over_a_wall_nearer_to_its_own_camera() {
    let Some(mut r) = renderer() else { return };
    let (mut scene, _) = against_a_wall(true);
    let mut v = view(&r);
    frame(&mut r, &mut v, &scene, &eye(0.0));
    let blurred = frame(&mut r, &mut v, &scene, &eye(8.0));
    assert!(v.post_fx.holds_blur(), "the blur ran");
    (scene, _) = against_a_wall(false);
    let mut v = view(&r);
    let sharp = frame(&mut r, &mut v, &scene, &eye(8.0));

    let on_gun = gun_inside(&sharp)
        .map(|p| diff(&blurred, &sharp, p))
        .max()
        .expect("the gun is on screen");
    let above = (0..H / 4).flat_map(|y| (0..W).map(move |x| (x, y)));
    let wall = above.map(|p| diff(&blurred, &sharp, p)).max().unwrap();
    eprintln!("[motion_blur] against a wall: gun differs by {on_gun}, wall edge by {wall}");
    assert!(on_gun <= 2, "the wall smeared over the gun: {on_gun}");
    assert!(wall >= 8, "the wall did not blur: {wall}");
}

pub(crate) fn gun_inside(frame: &[u8]) -> impl Iterator<Item = (u32, u32)> + '_ {
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
