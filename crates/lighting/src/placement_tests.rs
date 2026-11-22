use glam::Vec3;

use super::test_level::plan_static;
use super::{plan_light_probes, static_scene_aabb, ReflectionPlacement, EYE_HEIGHT};
use rusty_components::{ColliderComponent, ColliderShape};
use rusty_scene::Scene;

fn add_static_box(scene: &mut Scene, name: &str, pos: Vec3, size: Vec3) {
    let id = scene.add_entity(name.to_string());
    scene.world.set_static(id, true);
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = pos;
    }
    scene.world.set_collider(
        id,
        Some(ColliderComponent {
            shape: ColliderShape::Box { size },
            ..rusty_scene::create::default_collider()
        }),
    );
}

fn room_scene() -> Scene {
    let mut scene = Scene::new();
    add_static_box(
        &mut scene,
        "Floor",
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(20.0, 6.0, 20.0),
    );
    scene
}

#[test]
fn aabb_of_known_box_is_sensible() {
    let scene = room_scene();
    let (min, max) = static_scene_aabb(&scene).expect("static box bounds");
    assert!((min - Vec3::new(-10.0, -3.0, -10.0)).length() < 1e-4);
    assert!((max - Vec3::new(10.0, 3.0, 10.0)).length() < 1e-4);
}

#[test]
fn aabb_none_when_no_static_collider() {
    let scene = Scene::new();
    assert!(static_scene_aabb(&scene).is_none());
}

#[test]
fn light_probe_plan_falls_back_to_scene_aabb_without_nav() {
    let scene = room_scene();
    let plan = plan_light_probes(&scene, None, 5.0).expect("a plan from the static AABB");
    assert!((plan.min - Vec3::new(-10.0, -3.0, -10.0)).length() < 1e-4);
    assert!((plan.max - Vec3::new(10.0, 3.0, 10.0)).length() < 1e-4);
    assert_eq!(plan.spacing, 5.0);
}

#[test]
fn light_probe_spacing_is_clamped() {
    let scene = room_scene();
    let plan = plan_light_probes(&scene, None, 0.01).unwrap();
    assert!(plan.spacing >= 0.5, "tiny spacing must clamp up");
}

#[test]
fn light_probe_plan_none_on_empty_scene() {
    let scene = Scene::new();
    assert!(plan_light_probes(&scene, None, 4.0).is_none());
}

#[test]
fn light_probe_plan_uses_walkable_bounds() {
    let scene = room_scene();
    let walkable = (Vec3::new(-4.0, 0.0, -4.0), Vec3::new(4.0, 0.0, 4.0));
    let plan = plan_light_probes(&scene, Some(walkable), 4.0).unwrap();
    assert_eq!(plan.min, Vec3::new(-4.0, 0.0, -4.0));
    assert_eq!(
        plan.max,
        Vec3::new(4.0, 3.0, 4.0),
        "floor + 3 units of headroom"
    );
}

#[test]
fn light_probe_plan_clips_walkable_bounds_to_the_level() {
    let scene = room_scene();
    let walkable = (Vec3::new(-30.0, 2.0, -5.0), Vec3::new(30.0, 2.5, 5.0));
    let plan = plan_light_probes(&scene, Some(walkable), 4.0).unwrap();
    assert_eq!(plan.min, Vec3::new(-10.0, 2.0, -5.0));
    assert_eq!(plan.max, Vec3::new(10.0, 5.5, 5.0));
}

#[test]
fn light_probe_plan_none_when_walkable_misses_the_level() {
    let scene = room_scene();
    let walkable = (Vec3::new(50.0, 0.0, 50.0), Vec3::new(60.0, 0.0, 60.0));
    assert!(plan_light_probes(&scene, Some(walkable), 4.0).is_none());
}

#[test]
fn light_probe_plan_from_walkable_bounds_alone() {
    let walkable = (Vec3::new(-2.0, 1.0, -2.0), Vec3::new(2.0, 1.0, 2.0));
    let plan = plan_light_probes(&Scene::new(), Some(walkable), 4.0).unwrap();
    assert_eq!((plan.min, plan.max), (walkable.0, Vec3::new(2.0, 4.0, 2.0)));
}

#[test]
fn one_open_room_is_one_probe_without_box_projection() {
    let scene = room_scene();
    let plans = plan_static(&scene, &placement());
    assert_eq!(plans.len(), 1, "{plans:?}");
    let (amin, amax) = static_scene_aabb(&scene).unwrap();
    let p = plans[0];
    assert!(!p.box_projection);
    assert!(p.box_min.cmpge(amin - Vec3::splat(1e-4)).all(), "{p:?}");
    assert!(p.box_max.x <= amax.x + 1e-4 && p.box_max.z <= amax.z + 1e-4);
    assert!(p.box_max.x - p.box_min.x > 19.0, "the whole room: {p:?}");
    assert_eq!(p.box_min.y, 3.0, "the floor");
    assert!((p.position.y - (3.0 + EYE_HEIGHT)).abs() < 1e-5);
    assert!(p.box_max.y >= p.position.y, "open: up to the sky");
}

#[test]
fn reflection_plan_empty_without_static_geometry() {
    let scene = Scene::new();
    assert!(plan_static(&scene, &placement()).is_empty());
}

fn placement() -> ReflectionPlacement {
    ReflectionPlacement::default()
}

#[test]
fn placement_is_deterministic() {
    let a = room_scene();
    let b = room_scene();
    let walkable = Some((Vec3::new(-8.0, 0.0, -8.0), Vec3::new(8.0, 0.0, 8.0)));
    assert_eq!(
        plan_light_probes(&a, walkable, 4.0),
        plan_light_probes(&b, walkable, 4.0)
    );
    assert_eq!(plan_static(&a, &placement()), plan_static(&b, &placement()));
}
