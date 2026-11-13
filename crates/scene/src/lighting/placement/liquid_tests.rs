//! A liquid is not floor (#1737): a pit with water at deck level.

use glam::Vec3;

use super::test_level::{block, plan_static};
use super::test_sites::plan_on_nav;
use super::ReflectionPlacement;
use crate::authoring::{create_entity, Primitive};
use crate::Scene;
use rusty_components::{MaterialAsset, MaterialComponent, RenderMode};

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

/// A 2 × 1 plan, ground at 0 in both cells and a storey at 8.5 over the first.
fn two_storeys() -> super::FloorGrid {
    let open = Some(super::FloorCell {
        floor: 0.0,
        ceiling: f32::INFINITY,
    });
    super::FloorGrid {
        origin: glam::Vec2::ZERO,
        cell: 1.0,
        width: 2,
        depth: 1,
        cells: vec![open, open],
        upper: vec![(0, 8.5)],
        inset: 0.0,
    }
}

fn water(surface: f32) -> super::Liquid {
    super::Liquid {
        min: glam::Vec2::splat(-0.5),
        max: glam::Vec2::new(1.5, 0.5),
        surface,
    }
}

/// A rooftop pool drops the roof under it, never the ground storey further down.
#[test]
fn a_rooftop_pool_drops_only_the_floor_under_it() {
    let mut plan = two_storeys();
    plan.drown(&[water(9.0)]);
    assert!(plan.upper.is_empty(), "the pool's bottom goes");
    assert!(plan.cells[0].is_some(), "the ground storey stays");
    assert!(
        plan.cells[1].is_none(),
        "the cell with no storey: its ground is the bottom"
    );
}

/// A flat transparent quad just over the floor (a puddle, grime) is no pool.
#[test]
fn a_puddle_drops_no_floor() {
    let mut plan = two_storeys();
    plan.drown(&[water(0.01)]);
    assert_eq!(plan, two_storeys());
}
