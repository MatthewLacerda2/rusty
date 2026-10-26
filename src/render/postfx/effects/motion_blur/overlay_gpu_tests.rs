//! Depth order under a `DepthOnly` overlay camera (#1436), the standard FPS stack:
//! the overlay clears the depth buffer before the blur runs, so the blur orders
//! world against world from the depth each camera writes beside its velocity.

use glam::Vec3;

use super::gpu_tests::{diff, frame, streaky};
use crate::components::{CameraComponent, MaterialAsset, MaterialComponent};
use crate::render::motion::gpu_fixture::{cube, eye, pixel, project, renderer, view, H, W};
use crate::scene::{ClearFlags, Scene};

/// Give `id` a flat `colour` material, so the enemy and the wall tell apart.
pub(super) fn paint(scene: &mut Scene, id: u32, name: &str, colour: [f32; 3]) {
    let asset = MaterialAsset {
        base_color: colour,
        roughness: 1.0,
        ..MaterialAsset::default()
    };
    scene.materials.insert(name.into(), asset);
    let material = Some(MaterialComponent::new(name.to_string()));
    scene.world.set_material(id, material);
}

/// The layer the gun is drawn on, and only by the gun camera.
pub(super) const VIEWMODEL: u8 = 1;

/// A still wall 8.5 m out, an enemy cube 4.5 m out at `enemy_x`, and, with
/// `overlay`, a `DepthOnly` gun camera drawing only the viewmodel layer, whose gun
/// sits low and left, clear of the enemy; the world camera skips that layer. The
/// usual FPS stack. Returns the scene and the enemy.
fn arena(blur: bool, overlay: bool, enemy_x: f32) -> (Scene, u32) {
    let (mut scene, world) = streaky(blur);
    scene.world.camera_mut(world).unwrap().culling_mask = !(1 << VIEWMODEL);
    let wall = cube(&mut scene, Vec3::new(0.0, 0.0, -9.0));
    scene.world.transform_mut(wall).unwrap().scale = Vec3::new(8.0, 6.0, 1.0);
    paint(&mut scene, wall, "wall", [0.1, 0.1, 0.8]);
    let enemy = cube(&mut scene, Vec3::new(enemy_x, 0.0, -5.0));
    paint(&mut scene, enemy, "enemy", [0.9, 0.2, 0.1]);
    if overlay {
        let camera = scene.add_entity("GunCamera".to_string());
        let lens = CameraComponent {
            render_order: 1,
            clear_flags: ClearFlags::DepthOnly,
            culling_mask: 1 << VIEWMODEL,
            ..Default::default()
        };
        scene.world.set_camera(camera, Some(lens));
        let gun = cube(&mut scene, Vec3::new(-1.2, -0.8, -2.0));
        scene.world.transform_mut(gun).unwrap().scale = Vec3::splat(0.3);
        scene.world.set_layer(gun, VIEWMODEL);
        scene.set_parent(gun, Some(camera)).unwrap();
    }
    (scene, enemy)
}

/// The enemy moving one metre right past the wall, blurred: the final frame.
fn blurred(overlay: bool) -> Option<Vec<u8>> {
    let mut r = renderer()?;
    let cam = eye(0.0);
    let (mut scene, enemy) = arena(true, overlay, -0.5);
    let mut v = view(&r);
    frame(&mut r, &mut v, &scene, &cam);
    scene.world.transform_mut(enemy).unwrap().position = Vec3::new(0.5, 0.0, -5.0);
    let out = frame(&mut r, &mut v, &scene, &cam);
    assert!(v.post_fx.blur.is_some(), "the blur ran");
    Some(out)
}

#[test]
fn gpu_an_enemy_and_the_wall_blur_alike_with_and_without_a_gun_camera() {
    let Some(alone) = blurred(false) else { return };
    let gunned = blurred(true).expect("an adapter, as above");
    let mut r = renderer().unwrap();
    let (still, _) = arena(false, false, 0.5);
    let mut v = view(&r);
    let sharp = frame(&mut r, &mut v, &still, &eye(0.0));

    // The wall the enemy passes gets its full foreground smear: the four pixels
    // past each side of its front face.
    for (edge, out) in [(1.0, 1), (0.0, -1)] {
        let (x, y) = pixel(project(&eye(0.0), Vec3::new(edge, 0.0, -4.5)));
        let past = (1..=4).map(|i| ((x as i32 + out * i) as u32, y));
        let smear = past.map(|p| diff(&alone, &sharp, p)).max().unwrap();
        eprintln!("[motion_blur] the wall past x = {x} smears by {smear}");
        assert!(smear >= 8, "the enemy smears onto the wall at x = {x}");
    }
    // And the gun camera changes none of it: across the rows the enemy crosses
    // (the gun sits well below them), world against world orders the same.
    let rows = H / 2 - 4..=H / 2 + 4;
    let band = rows.flat_map(|y| (0..W).map(move |x| (x, y)));
    let gap = band.map(|p| diff(&gunned, &alone, p)).max().unwrap();
    eprintln!("[motion_blur] the gun camera changes the enemy's rows by {gap}");
    assert!(gap <= 1, "the gun camera changed the world's blur by {gap}");
}
