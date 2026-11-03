//! A liquid is not floor (#1737): a pit with water at deck level.

use glam::Vec3;

use super::test_level::{block, plan_static};
use super::test_sites::plan_on_nav;
use super::ReflectionPlacement;
use crate::components::{MaterialAsset, MaterialComponent, RenderMode};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::Scene;

/// A 6 × 6 m pit 1.5 m deep in the yard's deck, water at deck level.
fn pool() -> Scene {
    let mut scene = Scene::new();
    let (d, r) = (-1.5, 15.0);
    block(&mut scene, Vec3::new(-r, -0.2, -r), Vec3::new(r, 0.0, -3.0));
    block(&mut scene, Vec3::new(-r, -0.2, 3.0), Vec3::new(r, 0.0, r));
    block(
        &mut scene,
        Vec3::new(-r, -0.2, -3.0),
        Vec3::new(-3.0, 0.0, 3.0),
    );
    block(
        &mut scene,
        Vec3::new(3.0, -0.2, -3.0),
        Vec3::new(r, 0.0, 3.0),
    );
    block(
        &mut scene,
        Vec3::new(-3.0, d - 0.2, -3.0),
        Vec3::new(3.0, d, 3.0),
    );
    let water = create_entity(&mut scene, "Water", Some(Primitive::Plane));
    scene.world.set_static(water, true);
    if let Some(mut t) = scene.world.transform_mut(water) {
        t.position = Vec3::new(0.0, -0.15, 0.0);
        t.scale = Vec3::new(6.0 / 15.0, 1.0, 6.0 / 15.0);
    }
    let look = MaterialAsset {
        render_mode: RenderMode::Transparent,
        alpha: 0.5,
        ..MaterialAsset::default()
    };
    scene.materials.insert("water".into(), look);
    scene
        .world
        .set_material(water, Some(MaterialComponent::new("water")));
    scene
}

#[test]
fn no_capture_point_is_at_or_under_the_water() {
    let scene = pool();
    let in_pit = |p: Vec3| p.x.abs() < 3.0 && p.z.abs() < 3.0 && p.y <= 0.0;
    for plans in [
        plan_static(&scene, &ReflectionPlacement::default()),
        plan_on_nav(&scene),
    ] {
        assert!(!plans.is_empty());
        for p in &plans {
            assert!(!in_pit(p.position), "a capture in the pit: {p:?}");
            assert!(p.position.y > 1.0, "eye height over the deck: {p:?}");
        }
    }
}
