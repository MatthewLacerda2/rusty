//! Light that bounces (#1695's review). A `Mixed` spot's bounce off the floor reaches
//! a glossy ceiling from one small patch, directional (about 0.97) though no `Baked`
//! lamp lights it: no lobe. A `Baked` lamp in a room of bright walls supplies only
//! part of a texel's light, the walls the rest: its lobe still aims at the lamp, and
//! its highlight sits where the live lamp's does.

use std::f32::consts::{FRAC_PI_2, PI};

use glam::{Quat, Vec3};
use rusty::components::{LightMode, LightType, MaterialAsset};
use rusty::scene::{Camera, Scene};

use super::highlight::agree;
use super::{bake, lamp, most, peak, plane, point, shot};

fn matte(albedo: f32) -> MaterialAsset {
    MaterialAsset {
        base_color: [albedo; 3],
        roughness: 1.0,
        ..MaterialAsset::default()
    }
}

fn glossy() -> MaterialAsset {
    MaterialAsset {
        base_color: [0.5; 3],
        roughness: 0.25,
        ..MaterialAsset::default()
    }
}

/// A 4 x 4 m room 2.5 m high, every face inward, under a black sky: `floor` and
/// `ceiling` of the given materials, the walls white.
fn room(floor: MaterialAsset, ceiling: MaterialAsset) -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let s = 4.0 / 15.0;
    plane(&mut scene, "floor", Vec3::ZERO, Quat::IDENTITY, s, floor);
    let down = Quat::from_rotation_x(PI);
    plane(&mut scene, "ceiling", Vec3::Y * 2.5, down, s, ceiling);
    let walls = [
        (
            "wall_px",
            Vec3::new(2.0, 1.25, 0.0),
            Quat::from_rotation_z(FRAC_PI_2),
        ),
        (
            "wall_nx",
            Vec3::new(-2.0, 1.25, 0.0),
            Quat::from_rotation_z(-FRAC_PI_2),
        ),
        (
            "wall_pz",
            Vec3::new(0.0, 1.25, 2.0),
            Quat::from_rotation_x(-FRAC_PI_2),
        ),
        (
            "wall_nz",
            Vec3::new(0.0, 1.25, -2.0),
            Quat::from_rotation_x(FRAC_PI_2),
        ),
    ];
    for (name, at, facing) in walls {
        plane(&mut scene, name, at, facing, s, matte(0.9));
    }
    scene
}

/// `scene` baked (`directional` or not) under `name`, then shot from `camera`.
fn baked(mut scene: Scene, camera: &Camera, name: &str, directional: bool) -> Option<Vec<f32>> {
    bake(&mut scene, name, directional, super::DENSITY);
    shot(&scene, camera, name)
}

#[test]
fn a_mixed_spots_bounce_gives_a_glossy_ceiling_no_lobe() {
    let make = || {
        let metal = MaterialAsset {
            metallic: 1.0,
            base_color: [0.9; 3],
            ..glossy()
        };
        let mut scene = room(matte(0.9), metal);
        let spot = rusty::components::LightComponent {
            light_type: LightType::Spotlight,
            intensity: 4.0,
            outer_cone: 20.0,
            inner_cone: 15.0,
            ..point(LightMode::Mixed)
        };
        lamp(
            &mut scene,
            Vec3::Y * 2.3,
            Quat::from_rotation_x(-FRAC_PI_2),
            spot,
        );
        scene
    };
    let camera = Camera::new(Vec3::new(-1.7, 0.4, 0.0), 0.0, 40.0);
    let Some(lobed) = baked(make(), &camera, "ceiling_lobed", true) else {
        return;
    };
    let diffuse = baked(make(), &camera, "ceiling_diffuse", false).unwrap();
    let d = most(&lobed, &diffuse);
    eprintln!("[baked-spec] mixed bounce on the ceiling: at most {d:.1} apart");
    assert!(d <= 4.0, "a Mixed light's bounce gives no lobe: {d:.1}");
}

#[test]
fn a_lamp_in_a_bright_room_aims_its_lobe_at_itself() {
    let make = |mode| {
        let mut scene = room(glossy(), matte(0.9));
        lamp(&mut scene, Vec3::Y, Quat::IDENTITY, point(mode));
        scene
    };
    let camera = Camera::new(Vec3::new(-1.8, 1.5, 0.0), 0.0, -35.0);
    let Some(lobed) = baked(make(LightMode::Baked), &camera, "room_lobed", true) else {
        return;
    };
    let diffuse = baked(make(LightMode::Baked), &camera, "room_diffuse", false).unwrap();
    // `Mixed`: the lamp live over the same bounce, the truth to aim at.
    let live = baked(make(LightMode::Mixed), &camera, "room_live", false).unwrap();
    let pair = [peak(&lobed, &diffuse), peak(&live, &diffuse)];
    eprintln!(
        "[baked-spec] bright room: baked {:?}, live {:?}",
        pair[0], pair[1]
    );
    agree(pair, 2);
}
