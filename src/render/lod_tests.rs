//! LOD selection (#472): the level a view shows is picked by screen height, the other
//! levels' renderers are hidden, and the static-bake selection is always LOD0.

use glam::Vec3;

use super::*;
use crate::components::LodLevel;

/// A group at `z = -distance` (the camera looks down -Z from the origin) with one
/// renderer per level: ids 101, 102, 103 for LOD0..2.
fn scene_with_group(distance: f32) -> (Scene, u32) {
    let mut scene = Scene::new();
    let group = scene.world.spawn("Crate".to_string());
    scene.world.transform_mut(group).unwrap().position = Vec3::new(0.0, 0.0, -distance);
    let levels = [0.5, 0.1, 0.02]
        .into_iter()
        .zip(101..)
        .map(|(screen_height, r)| LodLevel {
            screen_height,
            renderers: vec![r],
        })
        .collect();
    let lod = LodGroupComponent { levels, size: 2.0 };
    scene.world.set_lod_group(group, Some(lod));
    (scene, group)
}

/// A camera at the origin looking down -Z with a 90° vertical fov, so the visible
/// height at distance `d` is exactly `2d` and a 2 m group covers `1/d` of the screen.
fn camera() -> Camera {
    let mut cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    cam.fov = 90.0;
    cam
}

fn hidden(sel: &LodSelection) -> Vec<u32> {
    let mut ids: Vec<u32> = (101..104).filter(|&id| sel.hides(id)).collect();
    ids.sort_unstable();
    ids
}

#[test]
fn screen_height_is_size_over_the_visible_height() {
    let (scene, group) = scene_with_group(4.0);
    let h = screen_height(2.0, &camera(), scene.world_matrix(group));
    assert!((h - 0.25).abs() < 1e-5, "2 m at 4 m under a 90° fov: {h}");
}

#[test]
fn each_distance_shows_exactly_one_level() {
    // 1/d: 1.0 → LOD0, 0.2 → LOD1, 0.04 → LOD2, 0.01 → culled.
    let cases = [
        (1.0, vec![102, 103]),
        (5.0, vec![101, 103]),
        (25.0, vec![101, 102]),
        (100.0, vec![101, 102, 103]),
    ];
    for (distance, expect) in cases {
        let (scene, _) = scene_with_group(distance);
        let sel = LodSelection::for_camera(&scene, &camera());
        assert_eq!(hidden(&sel), expect, "at {distance} m");
    }
}

#[test]
fn scaling_the_group_keeps_the_finer_level_farther() {
    let (mut scene, group) = scene_with_group(5.0);
    scene.world.transform_mut(group).unwrap().scale = Vec3::splat(3.0);
    let sel = LodSelection::for_camera(&scene, &camera());
    assert_eq!(hidden(&sel), vec![102, 103], "3x the size is LOD0 again");
}

#[test]
fn a_renderer_shared_with_the_shown_level_stays_visible() {
    let (mut scene, group) = scene_with_group(5.0);
    scene.world.lod_group_mut(group).unwrap().levels[2]
        .renderers
        .push(102);
    let sel = LodSelection::for_camera(&scene, &camera());
    assert!(!sel.hides(102), "LOD1 is shown and lists 102");
}

#[test]
fn the_static_bake_and_inactive_groups() {
    let (mut scene, group) = scene_with_group(100.0);
    assert_eq!(hidden(&LodSelection::finest(&scene)), vec![102, 103]);
    scene.world.set_active(group, false);
    let sel = LodSelection::for_camera(&scene, &camera());
    assert!(hidden(&sel).is_empty(), "an inactive group hides nothing");
}
