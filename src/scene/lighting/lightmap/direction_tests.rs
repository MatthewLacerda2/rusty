//! Directional lightmaps (#810): each texel records where its light comes from, the
//! filter smooths it without letting it grow past a unit vector, turning it off leaves
//! the colour lightmaps untouched, and the atlas packs it exactly like the colour.

use glam::Vec3;

use super::tests::{floor, settings, sun, wall};
use super::*;

/// The mean direction texel of `entity`'s lightmap.
fn mean_dir(maps: &[Lightmap], entity: u32) -> Vec3 {
    let map = maps.iter().find(|m| m.entity == entity).expect("baked");
    map.directions.iter().sum::<Vec3>() / map.directions.len() as f32
}

#[test]
fn a_baked_sun_points_the_wall_at_the_sun() {
    // The sun travels +X into the wall's front: its light arrives from -X.
    let scene = BakeScene {
        meshes: vec![floor(), wall()],
        lights: vec![sun(true)],
        sky: Vec3::ZERO,
    };
    let d = mean_dir(&bake(&scene, &settings()), 2);
    assert!(d.normalize().dot(-Vec3::X) > 0.95, "toward the sun: {d}");
    assert!(d.length() > 0.8, "nearly all of it from one way: {d}");
}

#[test]
fn an_open_floor_under_the_sky_points_up_but_not_fully() {
    // An even sky over a floor arrives from the whole hemisphere: the dominant
    // direction is up and its directionality the cosine-weighted mean, about 2/3.
    let scene = BakeScene {
        meshes: vec![floor()],
        lights: Vec::new(),
        sky: Vec3::splat(0.5),
    };
    let s = BakeSettings {
        samples: 256,
        ..settings()
    };
    let d = mean_dir(&bake(&scene, &s), 1);
    assert!(d.normalize().dot(Vec3::Y) > 0.98, "up: {d}");
    assert!((d.length() - 2.0 / 3.0).abs() < 0.1, "hemispherical: {d}");
}

#[test]
fn the_filter_smooths_directions_and_keeps_them_unit_bounded() {
    let scene = BakeScene {
        meshes: vec![floor(), wall()],
        lights: vec![sun(false)],
        sky: Vec3::new(0.2, 0.3, 0.4),
    };
    let raw = bake(&scene, &settings());
    let smooth = bake(
        &scene,
        &BakeSettings {
            filter_radius: 3,
            ..settings()
        },
    );
    let spread = |maps: &[Lightmap]| {
        let mean = mean_dir(maps, 1);
        let map = maps.iter().find(|m| m.entity == 1).unwrap();
        map.directions
            .iter()
            .map(|d| (*d - mean).length())
            .sum::<f32>()
    };
    assert!(spread(&smooth) < spread(&raw), "smoother than raw");
    for map in &smooth {
        assert!(map.directions.iter().all(|d| d.length() <= 1.0 + 1e-5));
    }
}

#[test]
fn turning_it_off_drops_directions_and_keeps_the_colour() {
    let scene = BakeScene {
        meshes: vec![floor(), wall()],
        lights: vec![sun(true)],
        sky: Vec3::new(0.2, 0.3, 0.4),
    };
    let on = bake(&scene, &settings());
    let off_settings = BakeSettings {
        directional: false,
        ..settings()
    };
    let off = bake(&scene, &off_settings);
    for (a, b) in on.iter().zip(&off) {
        assert_eq!(a.texels, b.texels, "the colour lightmap is the same");
        assert_eq!(a.directions.len(), a.texels.len());
        assert!(b.directions.is_empty());
    }
    assert!(pack(&off).directions.is_empty(), "no direction pages");
}

#[test]
fn direction_pages_share_the_colour_pages_layout() {
    let map = |entity, size: u32, value: f32| Lightmap {
        entity,
        size,
        texels: vec![Vec3::splat(value); (size * size) as usize],
        directions: vec![Vec3::Y * value; (size * size) as usize],
    };
    let atlas = pack(&[map(1, 16, 0.25), map(2, 8, 0.75)]);
    assert_eq!(atlas.directions.len(), atlas.pages.len());
    for (colour, dirs) in atlas.pages.iter().zip(&atlas.directions) {
        for (c, d) in colour.iter().zip(dirs) {
            assert_eq!(*d, Vec3::Y * c.x, "same texel, same place");
        }
    }
}
