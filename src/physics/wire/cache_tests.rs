use glam::Vec3;

use super::edges::unique_edges;
use super::{collider_wire, WireCache};
use crate::components::SharedMesh;
use crate::scene::{ColliderComponent, ColliderShape, Scene};

/// A scene with one entity whose triangle-mesh collider holds `positions` and
/// `indices` as its own mesh.
fn with_mesh(positions: Vec<[f32; 3]>, indices: Vec<u32>) -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("m".to_string());
    let mut col = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ONE);
    col.shape = ColliderShape::Mesh {
        convex: false,
        local_min: Vec3::ZERO,
        local_max: Vec3::ONE,
    };
    col.shared_mesh = Some(SharedMesh {
        positions,
        indices,
        ..Default::default()
    });
    scene.world.set_collider(id, Some(col));
    (scene, id)
}

/// A unit quad on the floor: four corners, two triangles.
fn quad() -> Vec<[f32; 3]> {
    vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ]
}

/// A triangle mesh draws each shared edge once, at its signed scale.
#[test]
fn a_mesh_draws_each_edge_once() {
    let (mut scene, id) = with_mesh(quad(), vec![0, 1, 2, 0, 2, 3]);
    let wire = collider_wire(&scene, id).expect("a trimesh");
    assert_eq!(wire.segments.len(), 5, "two triangles share their diagonal");
    scene.world.transform_mut(id).expect("live").scale = Vec3::new(-2.0, 1.0, 1.0);
    let wire = collider_wire(&scene, id).expect("mirrored");
    assert!(
        wire.segments.iter().flatten().any(|p| p.x == -2.0),
        "signed scale"
    );
}

/// An edge is one by its ends' positions: a seam split for its normals (the same
/// corner under two indices) draws once.
#[test]
fn a_split_seam_draws_once() {
    let mut points = quad();
    points.extend([[0.0, 0.0, 0.0], [1.0, 0.0, 1.0]]); // the diagonal's ends again
    let (scene, id) = with_mesh(points, vec![0, 1, 2, 4, 5, 3]);
    assert_eq!(
        collider_wire(&scene, id).expect("a trimesh").segments.len(),
        5
    );
    let tri = [Vec3::ZERO, Vec3::X, Vec3::Z];
    assert_eq!(unique_edges([tri, tri].into_iter()).len(), 3);
}

/// A mesh physics refuses (an index past its vertices, no area) is no collider,
/// so it draws nothing, however many of its edges are good.
#[test]
fn a_mesh_physics_refuses_draws_nothing() {
    let (scene, id) = with_mesh(quad(), vec![0, 1, 2, 0, 1, 9]);
    assert!(collider_wire(&scene, id).is_none(), "a bad index");
    let flat = vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
    let (scene, id) = with_mesh(flat, vec![0, 1, 2]);
    assert!(collider_wire(&scene, id).is_none(), "zero area");
}

/// A kept wire is built once: moving the entity places it again without a build;
/// a collider edit or a new scale builds it again, and a refused mesh stays refused
/// without being built (and warned about) every frame.
#[test]
fn the_cache_builds_only_on_a_change() {
    let (mut scene, id) = with_mesh(quad(), vec![0, 1, 2, 0, 2, 3]);
    let mut cache = WireCache::default();
    let frame = |cache: &mut WireCache, scene: &Scene| {
        cache.refresh(scene);
        cache.wire(scene, id).map(|w| w.segments.len())
    };
    assert_eq!(frame(&mut cache, &scene), Some(5));
    assert_eq!(frame(&mut cache, &scene), Some(5));
    scene.world.transform_mut(id).expect("live").position = Vec3::X;
    frame(&mut cache, &scene);
    assert_eq!(cache.builds(), 1, "a move places, never builds");
    scene.world.transform_mut(id).expect("live").scale = Vec3::splat(2.0);
    frame(&mut cache, &scene);
    assert_eq!(cache.builds(), 2, "a new scale builds");
    let mut col = scene.world.collider_mut(id).expect("collider");
    col.shared_mesh.as_mut().expect("mesh").indices = vec![0, 1, 9];
    drop(col);
    assert_eq!(frame(&mut cache, &scene), None, "an edit is seen");
    frame(&mut cache, &scene);
    assert_eq!(cache.builds(), 3, "a refused mesh is kept refused");
}

/// A child of a turning parent reads a world scale that drifts by float noise from
/// angle to angle: its wire is still the one built, never rebuilt.
#[test]
fn a_turning_parent_rebuilds_no_wire() {
    let (mut scene, id) = with_mesh(quad(), vec![0, 1, 2, 0, 2, 3]);
    let parent = scene.add_entity("p".to_string());
    scene.world.set_parent_id(id, Some(parent));
    scene.world.transform_mut(parent).expect("live").scale = Vec3::splat(1.7);
    scene.world.transform_mut(id).expect("live").scale = Vec3::new(1.3, 0.6, 2.1);
    let mut cache = WireCache::default();
    let mut scales = std::collections::BTreeSet::new();
    for step in 0..360 {
        let turn = glam::Quat::from_euler(glam::EulerRot::YXZ, step as f32 * 0.37, 0.2, 0.1);
        scene.world.transform_mut(parent).expect("live").rotation = turn;
        let scale = scene.world.world_pose(id).expect("live").scale;
        scales.insert(scale.to_array().map(f32::to_bits));
        cache.refresh(&scene);
        assert!(cache.wire(&scene, id).is_some());
    }
    assert!(
        scales.len() > 1,
        "the lossy scale drifts, or this tests nothing"
    );
    assert_eq!(cache.builds(), 1, "noise is no new scale");
}
