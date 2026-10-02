//! The agent-radius erosion (#277), on spans.

use super::super::super::test_support::{add_box, add_floor, bake_pinned, floors, ground};
use super::super::super::NavigationGraph;
use crate::scene::Scene;
use glam::Vec3;

/// A floor with tall (y = 3) one-cell wall strips at columns `left` and `right`.
fn corridor(left: i32, right: i32, radius: f32) -> NavigationGraph {
    let mut scene = Scene::new();
    add_floor(&mut scene, -5.0, 15.0, -5.0, 15.0);
    for x in [left as f32, right as f32] {
        add_box(
            &mut scene,
            Vec3::new(x - 0.25, 0.0, -5.0),
            Vec3::new(x + 0.25, 3.0, 15.0),
        );
    }
    scene.nav_settings.agent_radius = radius;
    bake_pinned(&mut scene)
}

#[test]
fn thin_passage_closes_under_erosion() {
    let g = corridor(3, 5, 1.0);
    for gz in 0..g.height {
        assert!(!g.is_walkable(4, gz), "passage cell (4,{gz}) eroded shut");
    }
    assert!(
        g.find_path(ground(&g, 1, 5), ground(&g, 8, 5)).is_none(),
        "sides cut apart"
    );
}

#[test]
fn wide_corridor_keeps_core_pulled_off_both_walls() {
    let g = corridor(1, 9, 1.0);
    assert!(!g.is_walkable(2, 5), "flush to the left wall");
    assert!(!g.is_walkable(8, 5), "flush to the right wall");
    for gx in 3..=7 {
        assert!(g.is_walkable(gx, 5), "core cell ({gx},5)");
    }
}

#[test]
fn radius_zero_is_exact_no_op() {
    let g = corridor(3, 5, 0.0);
    assert!(g.is_walkable(4, 5), "the one-cell passage stays open");
    assert!(g.is_walkable(2, 5) && g.is_walkable(0, 0));
}

#[test]
fn sub_cell_radius_still_erodes_one_cell() {
    let g = corridor(1, 9, 0.4);
    assert!(!g.is_walkable(2, 5), "ceil(0.4) = one cell");
    assert!(g.is_walkable(3, 5));
}

#[test]
fn larger_radius_erodes_superset() {
    let (small, large) = (corridor(1, 9, 1.0), corridor(1, 9, 2.0));
    for gz in 0..small.height {
        for gx in 0..small.width {
            if large.is_walkable(gx, gz) {
                assert!(
                    small.is_walkable(gx, gz),
                    "({gx},{gz}) kept at r=2, lost at r=1"
                );
            }
        }
    }
    assert!(!large.is_walkable(3, 5) && large.is_walkable(4, 5));
}

#[test]
fn stairs_are_not_an_edge() {
    // 0.25-high steps rising out of a wide floor.
    let mut scene = Scene::new();
    add_floor(&mut scene, -5.0, 15.0, -5.0, 15.0);
    for i in 1..=4 {
        let x = 2.0 + i as f32;
        add_box(
            &mut scene,
            Vec3::new(x - 0.5, 0.0, -5.0),
            Vec3::new(x + 0.5, 0.25 * i as f32, 15.0),
        );
    }
    scene.nav_settings.agent_radius = 1.0;
    let g = bake_pinned(&mut scene);
    // The climb x = 1..=5 links all the way through, so none of it erodes...
    for gx in 1..=5 {
        assert!(g.is_walkable(gx, 5), "({gx},5) kept across the stair run");
    }
    // ...while the top step ends in a 1.0 drop back to the floor: a ledge.
    assert_eq!(floors(&g, 6, 5), Vec::<f32>::new());
}
