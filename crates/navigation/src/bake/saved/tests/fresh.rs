use glam::Vec3;

use super::super::super::super::test_support::{assert_same_mesh, move_to};
use super::super::super::super::{NavigationGraph, Start};
use super::{baked, level};

#[test]
fn start_tracked_with_restores_bytes_a_fresh_graph_baked() {
    let (mut scene, crate_id) = level();
    let bytes = NavigationGraph::baked_bytes(&scene);
    let mut g = NavigationGraph::from_scene(&scene);
    assert_eq!(
        g.start_tracked_with(&mut scene, Some(&bytes)),
        Start::Restored
    );
    assert_same_mesh(&g, &baked(&scene));
    move_to(&mut scene, crate_id, Vec3::X * 6.0);
    assert_eq!(g.start_tracked_with(&mut scene, Some(&bytes)), Start::Baked);
    assert_same_mesh(&g, &baked(&scene));
}
