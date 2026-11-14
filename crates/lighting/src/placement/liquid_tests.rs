//! A liquid is not floor (#1737): a pit with water at deck level.

use glam::Vec3;

use super::test_level::plan_static;
use super::test_sites::pool;
use super::ReflectionPlacement;

#[test]
fn no_capture_point_is_at_or_under_the_water() {
    let scene = pool();
    let in_pit = |p: Vec3| p.x.abs() < 3.0 && p.z.abs() < 3.0 && p.y <= 0.0;
    // On the nav surface's floor: `navigation::probe_site::liquid_tests`.
    let plans = plan_static(&scene, &ReflectionPlacement::default());
    assert!(!plans.is_empty());
    for p in &plans {
        assert!(!in_pit(p.position), "a capture in the pit: {p:?}");
        assert!(p.position.y > 1.0, "eye height over the deck: {p:?}");
    }
}

/// A 2 × 1 plan, ground at 0 in both cells and a storey at 8.5 over the first.
fn two_storeys() -> rusty_scene::lighting::floor::FloorGrid {
    let open = Some(rusty_scene::lighting::floor::FloorCell {
        floor: 0.0,
        ceiling: f32::INFINITY,
    });
    rusty_scene::lighting::floor::FloorGrid {
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
    super::liquid::drown(&mut plan, &[water(9.0)]);
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
    super::liquid::drown(&mut plan, &[water(0.01)]);
    assert_eq!(plan, two_storeys());
}
