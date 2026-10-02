//! The headroom filter (#278): spans with less than `agent_height` of open space.

use super::super::super::test_support::{add_box, add_floor, bake_pinned, floors};
use super::super::super::NavigationGraph;
use crate::scene::Scene;
use glam::Vec3;

/// Floor over the whole grid, a slab over cells x = 3..=5, z = 4..=6 whose underside
/// is at `bottom`, baked at `agent_height` with no erosion.
fn overhang(bottom: f32, agent_height: f32) -> NavigationGraph {
    let mut scene = Scene::new();
    add_floor(&mut scene, 0.0, 10.0, 0.0, 10.0);
    add_box(
        &mut scene,
        Vec3::new(2.6, bottom, 3.6),
        Vec3::new(5.4, bottom + 0.5, 6.4),
    );
    scene.nav_settings.agent_radius = 0.0;
    scene.nav_settings.agent_height = agent_height;
    bake_pinned(&mut scene)
}

#[test]
fn low_overhang_drops_the_floor_beneath_but_keeps_its_top() {
    let g = overhang(1.0, 2.0);
    assert_eq!(floors(&g, 4, 5), vec![1.5], "only the slab top survives");
    assert_eq!(floors(&g, 8, 8), vec![0.0], "outside the footprint");
}

#[test]
fn clearance_exactly_the_agent_height_fits() {
    assert_eq!(floors(&overhang(2.0, 2.0), 4, 5), vec![0.0, 2.5]);
    assert_eq!(floors(&overhang(1.999, 2.0), 4, 5), vec![2.499]);
}

#[test]
fn taller_agent_drops_a_superset() {
    let short = overhang(2.5, 2.0);
    let tall = overhang(2.5, 3.0);
    assert_eq!(floors(&short, 4, 5), vec![0.0, 3.0]);
    assert_eq!(floors(&tall, 4, 5), vec![3.0]);
    assert!(tall.spans.len() < short.spans.len());
}

#[test]
fn span_ceiling_is_the_underside_above() {
    let g = overhang(2.5, 2.0);
    let ground = g.spans_at(4, 5)[0];
    assert_eq!(ground.ceiling, 2.5);
    assert!(g.spans_at(4, 5)[1].ceiling.is_infinite());
}
