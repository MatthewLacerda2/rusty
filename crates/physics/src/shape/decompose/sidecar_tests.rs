//! crates/physics/src/shape/decompose/sidecar_tests.rs — a dynamic Suzanne decomposes
//! once: an authoring build writes the pieces into her hull sidecar, and the next launch
//! builds her from it with no V-HACD run, to the same bits (#1940).

use glam::Vec3;

use super::tests::hull_bits;
use super::{forget_built, RUNS};
use crate::shape::fixture::prop;
use crate::PhysicsWorld;
use rusty_asset::import_cache::hulls::{file, save_pending, sidecar_of};
use rusty_core::hulls;
use rusty_scene::Scene;

type Mesh = (Vec<[f32; 3]>, Vec<u32>);

/// Suzanne's mesh, imported with her entry dropped from the import cache, so
/// every call reads her file and her sidecar afresh.
fn import(path: &std::path::Path) -> Mesh {
    rusty_asset::import_cache::forget(path);
    let import = rusty_asset::imported(path).expect("imports");
    let sub = &import.sub_meshes[0];
    let positions = sub.vertices.iter().map(|v| v.position).collect();
    (positions, sub.indices.clone())
}

/// Enter "Play": a dynamic Suzanne, her collider's shape, and the V-HACD runs.
fn play(mesh: &Mesh) -> (rapier3d::prelude::SharedShape, usize) {
    let before = RUNS.with(|r| r.get());
    let mut scene = Scene::new();
    let id = prop(&mut scene, mesh.clone(), Vec3::new(0.0, 2.0, 0.0), false);
    let world = PhysicsWorld::from_scene(&scene);
    let shape = world.colliders[world.id_to_collider[&id]]
        .shared_shape()
        .clone();
    (shape, RUNS.with(|r| r.get()) - before)
}

#[test]
fn a_second_launch_builds_suzanne_from_her_sidecar_with_no_vhacd() {
    let path = rusty_test_support::temp::dir().join("suzanne.obj");
    std::fs::copy(
        rusty_core::project::checkout().join("engine/models/suzanne.obj"),
        &path,
    )
    .unwrap();
    let sidecar = sidecar_of(&path).unwrap();
    let _ = std::fs::remove_file(&sidecar);

    let mesh = import(&path);
    let (first, runs) = play(&mesh);
    assert_eq!(runs, 1, "no sidecar yet: one decomposition");
    save_pending();
    // Whichever call took her pieces wrote them before this one returned.
    let key = hulls::key(mesh.0.iter().copied(), &mesh.1);
    let written = file::parse(&std::fs::read(&sidecar).expect("written")).unwrap();
    assert!(
        written.contains_key(&key),
        "an authoring build writes her sidecar"
    );

    // A fresh launch: nothing kept in memory, only the file on disk.
    hulls::forget(key);
    forget_built(key);
    let again = import(&path);
    let (second, runs) = play(&again);
    assert_eq!(runs, 0, "built from the sidecar");
    assert_eq!(hull_bits(&second), hull_bits(&first), "the same hulls");
    assert!(hull_bits(&first).len() > 8, "pieces, not one hull");
}
