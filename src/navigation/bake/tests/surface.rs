//! The single-floor surface: steps, walls, settings, determinism, what is baked.

use super::super::super::test_support::{
    add_box, add_box_with, add_floor, bake_pinned, floors, ground,
};
use super::super::super::NavigationGraph;
use crate::scene::Scene;
use glam::Vec3;

/// Floor at y = 0 plus a staircase rising 0.5 per step over cells x = 4..=6 at z = 2..6.
fn stairs() -> NavigationGraph {
    let mut scene = Scene::new();
    add_floor(&mut scene, 0.0, 10.0, 0.0, 10.0);
    for i in 1..=3 {
        let x = 3.0 + i as f32;
        add_box(
            &mut scene,
            Vec3::new(x - 0.25, 0.0, 2.0),
            Vec3::new(x + 0.25, 0.5 * i as f32, 6.0),
        );
    }
    scene.nav_settings.agent_radius = 0.0;
    bake_pinned(&mut scene)
}

#[test]
fn stairs_bake_one_rising_span_per_cell() {
    let g = stairs();
    assert_eq!(floors(&g, 3, 4), vec![0.0]);
    assert_eq!(floors(&g, 4, 4), vec![0.5]);
    assert_eq!(floors(&g, 6, 4), vec![1.5]);
    for gx in 3..=5 {
        assert!(
            g.link_to(ground(&g, gx, 4), gx + 1, 4).is_some(),
            "step {gx}"
        );
    }
}

#[test]
fn no_geometry_no_navmesh() {
    // #666's invariant: no implicit ground past the colliders, none in an empty scene.
    let g = NavigationGraph::from_scene(&Scene::new());
    assert!(g.spans.is_empty());
    let mut scene = Scene::new();
    add_floor(&mut scene, 2.0, 4.0, 2.0, 4.0);
    scene.nav_settings.agent_radius = 0.0;
    let g = bake_pinned(&mut scene);
    assert!(g.is_walkable(3, 3));
    assert!(
        !g.is_walkable(6, 6),
        "past the floor's edge there is nothing"
    );
}

#[test]
fn wall_top_is_a_span_nothing_links_to() {
    let mut scene = Scene::new();
    add_floor(&mut scene, 0.0, 10.0, 0.0, 10.0);
    add_box(
        &mut scene,
        Vec3::new(3.6, 0.0, 3.6),
        Vec3::new(4.4, 3.0, 4.4),
    );
    scene.nav_settings.agent_radius = 0.0;
    let g = bake_pinned(&mut scene);
    assert_eq!(
        floors(&g, 4, 4),
        vec![3.0],
        "the wall column is solid to its top"
    );
    assert!(
        g.link_to(ground(&g, 3, 4), 4, 4).is_none(),
        "cannot step up the wall"
    );
}

#[test]
fn dynamic_and_trigger_colliders_are_not_baked() {
    let mut scene = Scene::new();
    add_floor(&mut scene, 0.0, 10.0, 0.0, 10.0);
    add_box_with(
        &mut scene,
        Vec3::new(3.6, 0.0, 3.6),
        Vec3::new(4.4, 3.0, 4.4),
        false,
    );
    let trigger = add_box(
        &mut scene,
        Vec3::new(5.6, 0.0, 5.6),
        Vec3::new(6.4, 3.0, 6.4),
    );
    scene
        .world
        .collider_mut(trigger)
        .expect("collider")
        .is_trigger = true;
    scene.nav_settings.agent_radius = 0.0;
    let g = bake_pinned(&mut scene);
    assert_eq!(floors(&g, 4, 4), vec![0.0]);
    assert_eq!(floors(&g, 6, 6), vec![0.0]);
}

#[test]
fn bake_is_deterministic_and_order_independent() {
    let make = |reverse: bool| {
        let mut scene = Scene::new();
        let mut boxes = vec![
            (Vec3::new(0.0, -0.1, 0.0), Vec3::new(10.0, 0.0, 10.0)),
            (Vec3::new(3.6, 0.0, 3.6), Vec3::new(4.4, 0.7, 4.4)),
            (Vec3::new(2.0, 2.5, 2.0), Vec3::new(8.0, 2.7, 8.0)),
        ];
        if reverse {
            boxes.reverse();
        }
        for (min, max) in boxes {
            add_box(&mut scene, min, max);
        }
        scene.nav_settings.max_step = 0.8;
        scene.nav_settings.grid_spacing = 0.5;
        bake_pinned(&mut scene)
    };
    let (a, b) = (make(false), make(false));
    assert_eq!((&a.spans, &a.cell_start), (&b.spans, &b.cell_start));
    let c = make(true);
    assert_eq!((&a.spans, &a.cell_start), (&c.spans, &c.cell_start));
}
