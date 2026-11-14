//! The navmesh report: a deliberate unreachable island is counted, and the
//! colliders the bake read come heaviest first.

use glam::Vec3;

use super::{assert_common, scene_path};
use crate::bake::navmesh::bake_navmesh;
use crate::bake::report::{read, Kind};
use rusty_components::{ColliderComponent, ColliderShape};
use rusty_navigation::NavigationGraph;
use rusty_lighting::occlusion::test_level::block;
use rusty_scene::Scene;

/// A static box over `[min, max]` with a box collider.
fn solid(scene: &mut Scene, name: &str, min: Vec3, max: Vec3) -> u32 {
    let id = block(scene, name, min, max);
    let mut c = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    c.shape = ColliderShape::Box { size: Vec3::ONE };
    scene.world.set_collider(id, Some(c));
    id
}

/// A 20 m floor, and a 4 m platform off its edge 5 m up that nothing reaches.
fn level() -> Scene {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 0.5;
    solid(
        &mut scene,
        "Floor",
        Vec3::new(-10.0, -0.5, -10.0),
        Vec3::new(10.0, 0.0, 10.0),
    );
    let (lo, hi) = (Vec3::new(14.0, 4.5, -2.0), Vec3::new(18.0, 5.0, 2.0));
    solid(&mut scene, "Ledge", lo, hi);
    scene
}

#[test]
fn a_navmesh_bake_reports_the_unreachable_island() {
    let path = scene_path("navmesh");
    let mut scene = level();
    let mut nav = NavigationGraph::new(0.0, 1.0, 0.0, 1.0, 1.0);
    bake_navmesh(&mut scene, &mut nav, Some(&path)).unwrap();
    let report = read(&path, Kind::Navmesh).expect("the bake wrote its report");
    assert_common(&report, Kind::Navmesh);
    let islands = &report["islands"];
    assert_eq!(
        (islands["count"].as_u64(), islands["unreachable"].as_u64()),
        (Some(2), Some(1)),
        "{islands}"
    );
    let (floor, ledge) = (&islands["list"][0], &islands["list"][1]);
    assert_eq!(
        (floor["reachable"].as_bool(), ledge["reachable"].as_bool()),
        (Some(true), Some(false))
    );
    let ledge_y = ledge["centre"][1].as_f64().unwrap();
    assert!(
        (ledge_y - 5.0).abs() < 0.1,
        "the ledge's island is up on it: {ledge}"
    );
    // Its box runs along the cells' outer edges: whole cells wide, never zero.
    let cell = report["grid"]["cellSize"].as_f64().unwrap();
    let wide = (ledge["max"][0].as_f64().unwrap() - ledge["min"][0].as_f64().unwrap()) / cell;
    assert!(wide >= 1.0 && (wide - wide.round()).abs() < 0.02, "{ledge}");
    assert_eq!(report["threads"], 1, "the navmesh bakes on one thread");
    assert!(
        floor["squareMetres"].as_f64() > ledge["squareMetres"].as_f64(),
        "largest first"
    );
    assert_eq!(report["spans"].as_u64(), Some(nav.spans.len() as u64));
    assert_eq!(report["areas"][0]["name"], "Walkable");
    let colliders = &report["colliders"];
    assert_eq!(colliders["count"], 2);
    let heaviest = colliders["heaviest"].as_array().unwrap();
    let tris: Vec<u64> = heaviest
        .iter()
        .map(|c| c["triangles"].as_u64().unwrap())
        .collect();
    assert!(
        tris.windows(2).all(|w| w[0] >= w[1]) && tris[0] > 0,
        "{colliders}"
    );
    let size = std::fs::metadata(format!("{path}.navmesh")).unwrap().len();
    assert_eq!(report["outputBytes"].as_u64(), Some(size));
}

#[test]
fn a_bake_with_nowhere_to_write_writes_no_report() {
    let mut scene = level();
    let mut nav = NavigationGraph::new(0.0, 1.0, 0.0, 1.0, 1.0);
    assert_eq!(bake_navmesh(&mut scene, &mut nav, None), Ok(None));
}
