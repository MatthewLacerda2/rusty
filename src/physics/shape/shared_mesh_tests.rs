//! src/physics/shape/shared_mesh_tests.rs — a Mesh-shaped collider outlives its
//! entity's Mesh (#1657): `Scene.RemoveComponent(id, "Mesh")` hands the Mesh to the
//! collider (`ColliderComponent::shared_mesh`, Unity's `MeshCollider.sharedMesh`),
//! so physics and the navmesh still read it, after a save and load too.

use glam::Vec3;

use super::build::collider_inputs;
use super::triangles::collider_world_triangles;
use crate::components::mesh::{MeshComponent, Vertex};
use crate::components::ColliderComponent;
use crate::scene::authoring::{
    add_component, create_entity, remove_component, ComponentKind, Primitive,
};
use crate::scene::serialize::{apply_scene_data, to_scene_data};
use crate::scene::Scene;

/// An imported-style part: an `"Asset"` mesh (a 2×1×1 slab, not a
/// primitive) under the collider `instantiate_asset` gives it.
fn imported_part(scene: &mut Scene) -> u32 {
    let id = scene.add_entity("Part".to_string());
    let corners = [
        [-1.0, 0.0, -0.5],
        [1.0, 0.0, -0.5],
        [1.0, 1.0, -0.5],
        [-1.0, 1.0, -0.5],
        [-1.0, 0.0, 0.5],
        [1.0, 0.0, 0.5],
        [1.0, 1.0, 0.5],
        [-1.0, 1.0, 0.5],
    ];
    let vertices = corners
        .iter()
        .map(|&p| Vertex::new(Vec3::from_array(p), Vec3::Y, [0.0, 0.0]))
        .collect();
    #[rustfmt::skip]
    let indices = vec![
        0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 5, 4, 0, 1, 5,
        3, 6, 2, 3, 7, 6, 0, 7, 3, 0, 4, 7, 1, 6, 5, 1, 2, 6,
    ];
    let mesh = MeshComponent {
        asset_ref: Some("models/missing.glb::Slab".to_string()),
        ..MeshComponent::new("Asset", vertices, indices)
    };
    scene.world.set_mesh(id, Some(mesh));
    let collider = ColliderComponent::from_mesh_bounds(
        false,
        Vec3::new(-1.0, 0.0, -0.5),
        Vec3::new(1.0, 1.0, 0.5),
    );
    scene.world.set_collider(id, Some(collider));
    id
}

/// The x extent of the triangles physics collides with, if it builds any.
fn x_extent(scene: &Scene, id: u32) -> Option<(f32, f32)> {
    let tris = collider_world_triangles(scene, id)?;
    let xs = tris.vertices.iter().map(|v| v.x);
    Some(xs.fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x))))
}

#[test]
fn an_imported_part_keeps_colliding_without_its_mesh() {
    let mut scene = Scene::new();
    let id = imported_part(&mut scene);
    let before = x_extent(&scene, id).expect("builds with its Mesh");
    assert!(remove_component(&mut scene, id, ComponentKind::Mesh));
    assert!(!scene.world.has_mesh(id));
    let shared = scene.world.collider(id).and_then(|c| c.shared_mesh.clone());
    let shared = shared.expect("the collider keeps the Mesh");
    assert_eq!(
        shared.asset_ref.as_deref(),
        Some("models/missing.glb::Slab")
    );
    assert_eq!((shared.positions.len(), shared.indices.len()), (8, 36));
    assert!(collider_inputs(&scene.world, id).is_some_and(|i| i.mesh_geom.is_some()));
    // The same triangles: physics and the navmesh (both read these) lose nothing.
    assert_eq!(x_extent(&scene, id), Some(before));
    assert_eq!(before, (-1.0, 1.0));
}

#[test]
fn a_primitive_shaped_collider_keeps_nothing() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Box", Some(Primitive::Box));
    add_component(&mut scene, id, ComponentKind::Collider);
    remove_component(&mut scene, id, ComponentKind::Mesh);
    let collider = scene.world.collider(id).expect("collider stays");
    assert_eq!(collider.shared_mesh, None, "a Box collider needs no mesh");
    assert!(x_extent(&scene, id).is_some());
}

#[test]
fn the_shared_mesh_is_rebuilt_on_load() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Blockout", Some(Primitive::Box));
    let collider = ColliderComponent::from_mesh_bounds(true, Vec3::splat(-0.5), Vec3::splat(0.5));
    scene.world.set_collider(id, Some(collider));
    remove_component(&mut scene, id, ComponentKind::Mesh);
    let text = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    assert!(
        text.contains("\"shared_mesh\":{\"primitive_type\":\"Box\"}"),
        "{text}"
    );
    assert!(!text.contains("positions"), "only the reference is saved");
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, serde_json::from_str(&text).unwrap());
    let shared = loaded
        .world
        .collider(id)
        .and_then(|c| c.shared_mesh.clone());
    assert!(shared.is_some_and(|s| !s.positions.is_empty() && !s.indices.is_empty()));
    assert_eq!(x_extent(&loaded, id), x_extent(&scene, id));
    assert!(x_extent(&loaded, id).is_some());
}

#[test]
fn adding_a_mesh_back_drops_the_shared_one() {
    let mut scene = Scene::new();
    let id = imported_part(&mut scene);
    remove_component(&mut scene, id, ComponentKind::Mesh);
    add_component(&mut scene, id, ComponentKind::Mesh);
    let collider = scene.world.collider(id).expect("collider stays");
    assert_eq!(
        collider.shared_mesh, None,
        "the collider follows the Mesh again"
    );
    assert!(x_extent(&scene, id).is_some_and(|(lo, hi)| lo > -0.6 && hi < 0.6));
}

#[test]
fn a_bad_shared_mesh_loads_without_collision() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Blockout", Some(Primitive::Box));
    let collider = ColliderComponent::from_mesh_bounds(true, Vec3::splat(-0.5), Vec3::splat(0.5));
    scene.world.set_collider(id, Some(collider));
    remove_component(&mut scene, id, ComponentKind::Mesh);
    let text = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    let text = text.replace(
        "\"primitive_type\":\"Box\"}",
        "\"primitive_type\":\"Nope\"}",
    );
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, serde_json::from_str(&text).unwrap());
    assert_eq!(
        x_extent(&loaded, id),
        None,
        "skipped (and logged), never a panic"
    );
}
