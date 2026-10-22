//! src/physics/query/convex_fixture.rs — a convex mesh collider turned and scaled
//! the way #1211 reported, for the point-query tests in Rust and in Lua.

use glam::{Quat, Vec3};

use crate::components::mesh::Vertex;
use crate::components::{ColliderComponent, MeshComponent};
use crate::scene::authoring::{primitive_geometry, Primitive};
use crate::scene::Scene;

/// The Cylinder primitive moved off its origin, the conformance suite's off-centre
/// mesh: rings of coplanar points, the classic quickhull degeneracy.
pub(in crate::physics) fn off_centre_cylinder() -> (Vec<Vertex>, Vec<u32>) {
    let (mut vertices, indices) = primitive_geometry(Primitive::Cylinder).expect("a mesh");
    let offset = Vec3::new(0.3, 0.2, -0.1);
    vertices
        .iter_mut()
        .for_each(|v| v.position = (Vec3::from(v.position) + offset).to_array());
    (vertices, indices)
}

/// [`off_centre_cylinder`] as a convex mesh collider on an entity posed the way
/// #1211 reported, and that pose's world-space vertices.
pub(crate) fn turned_hull() -> (Scene, u32, Vec<Vec3>) {
    let (vertices, indices) = off_centre_cylinder();
    let locals: Vec<Vec3> = vertices.iter().map(|v| Vec3::from(v.position)).collect();
    let (min, max) = locals.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), &p| {
        (lo.min(p), hi.max(p))
    });
    let mut scene = Scene::new();
    let id = scene.add_entity("Hull".to_string());
    let (pos, rot, scale) = (
        Vec3::new(4.0, -1.0, 2.0),
        Quat::from_xyzw(0.5, -0.5, 0.5, 0.5),
        Vec3::splat(999.9999),
    );
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.rotation, t.scale) = (pos, rot, scale);
    drop(t);
    scene.world.set_static(id, true);
    let mesh = MeshComponent::new("Cylinder", vertices, indices);
    scene.world.set_mesh(id, Some(mesh));
    let collider = ColliderComponent::from_mesh_bounds(true, min, max);
    scene.world.set_collider(id, Some(collider));
    let world = locals.iter().map(|&p| pos + rot * (p * scale)).collect();
    (scene, id, world)
}
