//! Stacked floors (#454): several spans per cell, linked floor by floor.

use super::super::super::test_support::{add_box, add_floor, bake_pinned, floors, ground};
use crate::scene::Scene;
use glam::Vec3;

/// Ground floor plus a 3 m-high upper deck over x = 2..8, z = 2..8 (0.2 thick).
fn two_floors() -> Scene {
    let mut scene = Scene::new();
    add_floor(&mut scene, 0.0, 10.0, 0.0, 10.0);
    add_box(
        &mut scene,
        Vec3::new(2.0, 2.8, 2.0),
        Vec3::new(8.0, 3.0, 8.0),
    );
    scene.nav_settings.agent_radius = 0.0;
    scene
}

#[test]
fn a_deck_over_the_ground_keeps_both_floors() {
    let g = bake_pinned(&mut two_floors());
    assert_eq!(floors(&g, 5, 5), vec![0.0, 3.0]);
    assert!(
        (g.spans_at(5, 5)[0].ceiling - 2.8).abs() < 1e-5,
        "ground ceiling is the deck's underside"
    );
    assert_eq!(floors(&g, 9, 9), vec![0.0]);
}

#[test]
fn ground_walks_under_the_deck_and_the_deck_stays_apart() {
    let g = bake_pinned(&mut two_floors());
    let under = g
        .find_path(ground(&g, 0, 5), ground(&g, 9, 5))
        .expect("under the deck");
    assert!(under.iter().all(|s| g.spans[s.index as usize].y == 0.0));
    let deck = g.span_range(5, 5).start as u32 + 1;
    let deck = crate::navigation::SpanRef {
        gx: 5,
        gz: 5,
        index: deck,
    };
    assert!(
        g.find_path(ground(&g, 0, 5), deck).is_none(),
        "no stairs yet"
    );
}

#[test]
fn erosion_pulls_the_deck_back_from_its_own_edge() {
    let mut scene = two_floors();
    scene.nav_settings.agent_radius = 1.0;
    let g = bake_pinned(&mut scene);
    assert_eq!(floors(&g, 2, 5).len(), 1, "deck edge eroded, ground kept");
    assert_eq!(floors(&g, 5, 5), vec![0.0, 3.0]);
}
