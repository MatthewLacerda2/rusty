//! The bake's contract: deterministic per seed, bounce reaches what direct light cannot, the light
//! modes decide what is baked, and the sky bakes to the flat ambient an open surface shows.

use glam::{Vec2, Vec3};

use super::*;

pub(super) fn quad(entity: u32, c: [Vec3; 4], albedo: Vec3) -> BakeMesh {
    let n = (c[1] - c[0]).cross(c[2] - c[0]).normalize();
    BakeMesh {
        entity,
        positions: c.to_vec(),
        normals: vec![n; 4],
        lightmap_uvs: vec![Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        indices: vec![0, 1, 2, 0, 2, 3],
        runs: vec![BakeRun {
            end: 6,
            albedo,
            emissive: Vec3::ZERO,
            occludes: true,
        }],
        ..Default::default()
    }
}

pub(super) fn floor() -> BakeMesh {
    let p = |x, z| Vec3::new(x, 0.0, z);
    quad(
        1,
        [p(-2.0, 2.0), p(2.0, 2.0), p(2.0, -2.0), p(-2.0, -2.0)],
        Vec3::splat(0.5),
    )
}

pub(super) fn wall() -> BakeMesh {
    let p = |y, z| Vec3::new(2.0, y, z);
    quad(
        2,
        [p(0.0, -2.0), p(0.0, 2.0), p(4.0, 2.0), p(4.0, -2.0)],
        Vec3::ONE,
    )
}

pub(super) fn sun(mode_baked: bool) -> BakeLight {
    BakeLight {
        shape: LightShape::Directional { direction: Vec3::X },
        radiance: Vec3::splat(3.0),
        bakes_direct: mode_baked,
        mask_channel: None,
    }
}

pub(super) fn settings() -> BakeSettings {
    BakeSettings {
        texels_per_unit: 2.0,
        samples: 32,
        bounces: 2,
        seed: 7,
        max_lightmap_size: 32,
        denoiser: Denoiser::Filter,
        filter_radius: 0,
        directional: true,
        backface_tolerance: BakeSettings::DEFAULT_BACKFACE_TOLERANCE,
        ..Default::default()
    }
}

pub(super) fn mean(maps: &[Lightmap], entity: u32) -> f32 {
    let map = maps.iter().find(|m| m.entity == entity).expect("baked");
    map.texels().iter().map(|t| t.x).sum::<f32>() / map.texels().len() as f32
}

#[test]
fn same_seed_gives_identical_texels() {
    let scene = BakeScene {
        shadowmask: Default::default(),
        meshes: vec![floor(), wall()],
        lights: vec![sun(false)],
        sky: Vec3::new(0.3, 0.4, 0.5).into(),
        diffuse: Default::default(),
    };
    let a = bake(&scene, &settings());
    assert_eq!(a, bake(&scene, &settings()), "same seed, same texels");
    let reseeded = BakeSettings {
        seed: 8,
        ..settings()
    };
    assert_ne!(a, bake(&scene, &reseeded), "the seed drives the samples");
}

#[test]
fn a_mixed_light_reaches_the_floor_only_by_bounce() {
    // The sun travels along the floor (cos 0): it cannot light it directly, only off
    // the wall it hits head-on. No sky, so anything on the floor is bounce.
    let alone = BakeScene {
        shadowmask: Default::default(),
        meshes: vec![floor()],
        lights: vec![sun(false)],
        sky: Vec3::ZERO.into(),
        diffuse: Default::default(),
    };
    assert_eq!(
        mean(&bake(&alone, &settings()), 1),
        0.0,
        "no wall, nothing to bounce"
    );

    let with_wall = BakeScene {
        meshes: vec![floor(), wall()],
        ..alone
    };
    let maps = bake(&with_wall, &settings());
    assert!(
        mean(&maps, 1) > 0.01,
        "the wall bounces the sun onto the floor"
    );
    // The wall only gets the floor's bounce back, far below its direct E / π ≈ 0.95.
    let wall = mean(&maps, 2);
    assert!(
        wall < 0.2,
        "a Mixed light's direct light is not baked: {wall}"
    );
}

#[test]
fn a_baked_light_bakes_its_direct_light_too() {
    let scene = |baked| BakeScene {
        shadowmask: Default::default(),
        meshes: vec![floor(), wall()],
        lights: vec![sun(baked)],
        sky: Vec3::ZERO.into(),
        diffuse: Default::default(),
    };
    let wall_mixed = mean(&bake(&scene(false), &settings()), 2);
    let wall_baked = mean(&bake(&scene(true), &settings()), 2);
    // The wall faces the sun head-on: E = 3, stored as E / π.
    let direct = 3.0 / std::f32::consts::PI;
    assert!(
        (wall_baked - wall_mixed - direct).abs() < 0.05,
        "{wall_baked} vs {wall_mixed}"
    );
}
