//! Persistent tiles (#1711), as the static cache sees them: a blinking light
//! re-bakes no tile, a camera move within the size band changes no tile, and a
//! light's first frame has its tiles.

use glam::Vec3;

use super::cache::StaticTiles;
use super::plan::{AtlasPlan, MIN_TILE};
use super::slots::{Frame, ShadowSlots, SETTLE_FRAMES};
use super::Tile;
use crate::components::LightComponent;
use crate::render::clusters::LocalLight;
use crate::scene::{Camera, LightType, Scene};

/// A shadowed light of range 4 at `at`, shining down -Z.
pub(super) fn light(kind: LightType, at: Vec3, intensity: f32) -> LocalLight {
    let light = LightComponent {
        light_type: kind,
        intensity,
        range: 4.0,
        inner_cone: 20.0,
        outer_cone: 30.0,
        cast_shadows: true,
        ..crate::scene::authoring::default_light()
    };
    LocalLight::new(at, Vec3::NEG_Z, &light).expect("a local light")
}

/// One frame of `slots` for `lights` keyed by `ids`, from `eye` looking down -Z;
/// every id alive.
pub(super) fn step(
    slots: &mut ShadowSlots,
    lights: &[LocalLight],
    ids: &[u32],
    eye: Vec3,
) -> AtlasPlan {
    let requests: Vec<_> = lights.iter().map(|&l| (l, true)).collect();
    let frame = Frame {
        lights: &requests,
        ids,
        view_proj: Camera::new(eye, -90.0, 0.0).build_view_projection(16.0 / 9.0),
        eye,
    };
    slots.plan(None, &frame, |_| true)
}

/// Light `i`'s tiles in `plan`.
fn tiles_of(plan: &AtlasPlan, i: usize) -> Vec<Tile> {
    let (index, (_, first)) = (plan.shadows.iter().enumerate())
        .find(|(_, s)| s.0 == i)
        .expect("light shadowed");
    let end = (plan.shadows.get(index + 1)).map_or(plan.tiles.len(), |s| s.1 as usize);
    plan.tiles[*first as usize..end].to_vec()
}

#[test]
fn a_blinking_light_rebakes_no_tile() {
    let scene = (Scene::new().id(), 0, 0);
    let alarm = light(LightType::Spotlight, Vec3::new(0.0, 0.0, -6.0), 4.0);
    let mut lights = [
        alarm,
        light(LightType::Point, Vec3::new(2.0, 0.0, -8.0), 1.0),
        light(LightType::Spotlight, Vec3::new(-2.0, 0.0, -10.0), 0.5),
    ];
    let ids = [10, 20, 30];
    let (mut slots, mut cache) = (ShadowSlots::default(), StaticTiles::default());
    let first = step(&mut slots, &lights, &ids, Vec3::ZERO);
    cache.commit(scene, first.tiles.iter().copied());
    // On, off for a second, on again, twice: the off frames shade it unshadowed.
    for frame in 0..140 {
        lights[0].intensity = if (frame / 35) % 2 == 0 { 0.0 } else { 4.0 };
        let plan = step(&mut slots, &lights, &ids, Vec3::ZERO);
        assert!(
            cache.stale(scene, &plan.tiles).is_empty(),
            "frame {frame} re-bakes"
        );
        for i in 1..3 {
            assert_eq!(tiles_of(&plan, i), tiles_of(&first, i), "frame {frame}");
        }
        if lights[0].intensity > 0.0 {
            assert_eq!(tiles_of(&plan, 0), tiles_of(&first, 0), "frame {frame}");
        } else {
            assert_eq!(plan.shadows.len(), 2, "an unlit light draws no shadow");
        }
    }
}

#[test]
fn a_camera_move_within_the_band_changes_no_tile() {
    let lamp = [light(LightType::Point, Vec3::new(0.0, 0.0, -10.0), 1.0)];
    let mut slots = ShadowSlots::default();
    // Six metres away the light wants 341 texels: a 256 tile.
    let near = Vec3::new(0.0, 0.0, -4.0);
    let first = step(&mut slots, &lamp, &[1], near);
    assert_eq!(first.tiles[0].size, 256);
    // Ten metres away it wants 205, a 128 step, but within the band of 256.
    for frame in 0..100 {
        let eye = if frame % 2 == 0 { Vec3::ZERO } else { near };
        let plan = step(&mut slots, &lamp, &[1], eye);
        assert_eq!(plan.tiles, first.tiles, "frame {frame}");
    }
    // Forty metres away it wants 51: past the band, the tile shrinks once the
    // want has stayed out for SETTLE_FRAMES frames.
    let far = Vec3::new(0.0, 0.0, 30.0);
    for frame in 1..SETTLE_FRAMES {
        let plan = step(&mut slots, &lamp, &[1], far);
        assert_eq!(plan.tiles, first.tiles, "frame {frame} settles early");
    }
    let settled = step(&mut slots, &lamp, &[1], far);
    assert_eq!(settled.tiles[0].size, MIN_TILE);
}

#[test]
fn a_light_has_its_tiles_on_its_first_frame() {
    let scene = (Scene::new().id(), 0, 0);
    let a = light(LightType::Point, Vec3::new(0.0, 0.0, -6.0), 1.0);
    let b = light(LightType::Spotlight, Vec3::new(1.0, 0.0, -7.0), 2.0);
    let (mut slots, mut cache) = (ShadowSlots::default(), StaticTiles::default());
    let before = step(&mut slots, &[a], &[1], Vec3::ZERO);
    cache.commit(scene, before.tiles.iter().copied());
    let plan = step(&mut slots, &[a, b], &[1, 2], Vec3::ZERO);
    let tiles = tiles_of(&plan, 1);
    assert_eq!(
        tiles.len(),
        1,
        "the spotlight is shadowed on its first frame"
    );
    let stale = cache.stale(scene, &plan.tiles);
    assert_eq!(stale.len(), 1, "and baked that frame, alone: {stale:?}");
    assert_eq!(plan.tiles[stale[0]], tiles[0]);
    assert_eq!(
        tiles_of(&plan, 0),
        tiles_of(&before, 0),
        "the other stays put"
    );
}
