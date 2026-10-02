//! The atlas plan (#468): only lights that cast and reach the view get tiles, the
//! most important first, tiles never overlap, and what does not fit is counted.

use glam::{Quat, Vec3, Vec4};

use super::plan::{plan, AtlasPlan, MAX_TILE, MIN_TILE};
use super::ATLAS_SIZE;
use crate::components::{LightComponent, TransformComponent};
use crate::render::clusters::LocalLight;
use crate::scene::{Camera, LightType};

/// A camera at the origin looking down -Z.
fn camera() -> Camera {
    Camera::new(Vec3::ZERO, -90.0, 0.0)
}

fn light(kind: LightType, at: Vec3, intensity: f32) -> LocalLight {
    let transform = TransformComponent {
        position: at,
        rotation: Quat::IDENTITY,
        ..Default::default()
    };
    let light = LightComponent {
        light_type: kind,
        color: Vec3::ONE,
        intensity,
        range: 4.0,
        inner_cone: 20.0,
        outer_cone: 30.0,
        cast_shadows: true,
    };
    LocalLight::new(&transform, &light).expect("a local light")
}

fn plan_for(lights: &[(LocalLight, bool)]) -> AtlasPlan {
    plan(
        lights,
        camera().build_view_projection(16.0 / 9.0),
        Vec3::ZERO,
    )
}

#[test]
fn only_casting_lights_in_view_get_tiles() {
    let ahead = Vec3::new(0.0, 0.0, -6.0);
    let behind = Vec3::new(0.0, 0.0, 30.0);
    let lights = [
        (light(LightType::Spotlight, ahead, 1.0), true),
        (light(LightType::Point, ahead, 1.0), true),
        (light(LightType::Point, ahead, 1.0), false),
        (light(LightType::Point, behind, 1.0), true),
    ];
    let p = plan_for(&lights);
    let owners: Vec<usize> = p.shadows.iter().map(|s| s.0).collect();
    assert_eq!(owners.len(), 2, "{owners:?}");
    assert!(owners.contains(&0) && owners.contains(&1));
    assert_eq!(p.tiles.len(), 1 + 6, "a spotlight's tile and a point's six");
    assert_eq!(p.dropped, 0, "out of view or not casting is not dropped");
}

#[test]
fn tiles_stay_inside_the_atlas_and_never_overlap() {
    let lights: Vec<_> = (0..12)
        .map(|i| {
            (
                light(LightType::Point, Vec3::new(i as f32 - 6.0, 0.0, -8.0), 1.0),
                true,
            )
        })
        .collect();
    let p = plan_for(&lights);
    assert!(!p.tiles.is_empty());
    let rect = |t: &super::Tile| (t.origin[0], t.origin[1], t.size);
    for (i, a) in p.tiles.iter().enumerate() {
        let (x, y, s) = rect(a);
        assert!(s.is_power_of_two() && (MIN_TILE..=MAX_TILE).contains(&s));
        assert!(
            x + s <= ATLAS_SIZE && y + s <= ATLAS_SIZE,
            "tile {i} spills out"
        );
        for b in &p.tiles[i + 1..] {
            let (bx, by, bs) = rect(b);
            let apart = x + s <= bx || bx + bs <= x || y + s <= by || by + bs <= y;
            assert!(apart, "tiles {:?} and {:?} overlap", rect(a), rect(b));
        }
    }
    assert!(p.texels() <= u64::from(ATLAS_SIZE).pow(2));
}

#[test]
fn the_least_important_lights_are_dropped_and_counted() {
    // 60 point lights in view want 360 tiles; the atlas holds 256 of the smallest.
    let lights: Vec<_> = (0..60)
        .map(|i| {
            let at = Vec3::new((i % 10) as f32 - 5.0, (i / 10) as f32 - 3.0, -40.0);
            (light(LightType::Point, at, 1.0 + i as f32), true)
        })
        .collect();
    let p = plan_for(&lights);
    assert!(p.dropped > 0, "{} shadowed", p.shadows.len());
    assert_eq!(p.shadows.len() as u32 + p.dropped, 60);
    // The brightest (here, the last) win.
    let kept_min = p.shadows.iter().map(|s| s.0).min().unwrap();
    assert_eq!(kept_min, p.dropped as usize, "the dimmest were dropped");
}

#[test]
fn a_near_light_gets_a_bigger_tile_than_a_far_one() {
    let near = (
        light(LightType::Spotlight, Vec3::new(0.0, 0.0, -2.0), 1.0),
        true,
    );
    let far = (
        light(LightType::Spotlight, Vec3::new(0.0, 0.0, -60.0), 1.0),
        true,
    );
    let p = plan_for(&[far, near]);
    let size_of = |owner| {
        let (_, tile) = p.shadows.iter().find(|s| s.0 == owner).unwrap();
        p.tiles[*tile as usize].size
    };
    assert_eq!(size_of(1), MAX_TILE);
    assert_eq!(size_of(0), MIN_TILE);
}

#[test]
fn a_point_lights_faces_each_see_their_own_axis() {
    let at = Vec3::new(1.0, 2.0, -5.0);
    let p = plan_for(&[(light(LightType::Point, at, 1.0), true)]);
    let axes = [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ];
    for (face, axis) in axes.iter().enumerate() {
        let clip = p.tiles[face].view_proj * (at + *axis * 2.0).extend(1.0);
        let ndc = clip / clip.w;
        let centred = ndc.truncate().truncate().length() < 1e-4;
        assert!(
            centred && ndc.z > 0.0 && ndc.z < 1.0,
            "face {face}: {ndc:?}"
        );
    }
    // A spotlight's one view looks down its cone.
    let spot = light(LightType::Spotlight, at, 1.0);
    let p = plan_for(&[(spot, true)]);
    let clip = p.tiles[0].view_proj * Vec4::from((at + Vec3::NEG_Z, 1.0));
    assert!((clip / clip.w).truncate().truncate().length() < 1e-4);
}
