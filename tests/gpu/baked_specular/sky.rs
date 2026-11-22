//! A sky in the bake: an even sky lights a wall more from above than below, so its texels are
//! directional (about 0.79) with no lamp; the lobe once took that for a light, a highlight near
//! 14x the sky on glossy metal. The lamp share makes the sky give none; a lamp keeps its own.

use glam::{Quat, Vec3};
use rusty::components::MaterialAsset;
use rusty::scene::{Camera, Scene};

use super::highlight::{agree, highlights};
use super::{bake, most, plane, scene, shot};

pub(super) fn sky(scene: &mut Scene, radiance: f32) {
    scene.ambient_color = Vec3::ONE;
    scene.ambient_intensity = radiance;
}

fn wall_under_sky() -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    sky(&mut scene, 0.5);
    let ground = MaterialAsset {
        base_color: [0.02; 3],
        ..MaterialAsset::default()
    };
    plane(
        &mut scene,
        "ground",
        Vec3::ZERO,
        Quat::IDENTITY,
        1.0,
        ground,
    );
    let metal = MaterialAsset {
        base_color: [0.9; 3],
        metallic: 1.0,
        roughness: 0.25,
        ..MaterialAsset::default()
    };
    let facing = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    plane(
        &mut scene,
        "wall",
        Vec3::new(1.0, 1.5, 0.0),
        facing,
        0.2,
        metal,
    );
    scene
}

#[test]
fn a_sky_lit_metal_wall_shows_no_lobe() {
    // From below the wall's middle, looking up: where a lobe tilted up off the
    // wall's normal would be mirrored.
    let camera = Camera::new(Vec3::new(-2.0, 0.4, 0.0), 0.0, 18.0);
    let shoot = |directional: bool| {
        let mut scene = wall_under_sky();
        let name = format!("sky_wall_{directional}");
        bake(&mut scene, &name, directional, super::DENSITY);
        shot(&scene, &camera, &name)
    };
    let Some(lobed) = shoot(true) else {
        return;
    };
    let diffuse = shoot(false).unwrap();
    let d = most(&lobed, &diffuse);
    eprintln!("[baked-spec] sky wall: at most {d:.1} apart");
    assert!(d <= 4.0, "a sky with no lamp gives no lobe: {d:.1}");
}

#[test]
fn a_lamp_under_a_sky_keeps_its_highlight() {
    let make = |mode| {
        let mut s = scene(mode, 0.25);
        sky(&mut s, 0.3);
        s
    };
    if let Some(pair) = highlights(make, "sky_lamp") {
        agree(pair, 2);
    }
}
