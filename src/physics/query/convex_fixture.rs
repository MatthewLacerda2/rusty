//! src/physics/query/convex_fixture.rs — convex mesh colliders for the point-query
//! tests in Rust and in Lua: one turned and scaled the way #1211 reported, and a
//! flat one.

use glam::{Quat, Vec3};

use crate::components::mesh::Vertex;
use crate::components::{ColliderComponent, MeshComponent};
use crate::physics::shape::fixture::off_centre_cylinder;
use crate::scene::authoring::{primitive_geometry, Primitive};
use crate::scene::Scene;

/// `off_centre_cylinder` as a convex mesh collider on an entity posed the way
/// #1211 reported, and that pose's world-space vertices.
pub(crate) fn turned_hull() -> (Scene, u32, Vec<Vec3>) {
    let (pos, rot) = (
        Vec3::new(4.0, -1.0, 2.0),
        Quat::from_xyzw(0.5, -0.5, 0.5, 0.5),
    );
    let (vertices, indices) = off_centre_cylinder();
    convex_collider(
        "Cylinder",
        (vertices, indices),
        (pos, rot, Vec3::splat(999.9999)),
    )
}

/// The Plane primitive (15 × 15, at y = 0) as a convex mesh collider at the
/// origin: a hull with no volume.
pub(super) fn flat_hull() -> (Scene, u32) {
    let (vertices, indices) = primitive_geometry(Primitive::Plane).expect("a mesh");
    let pose = (Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
    let (scene, id, _) = convex_collider("Plane", (vertices, indices), pose);
    (scene, id)
}

/// A static entity drawing the primitive `name`'s `vertices` with a convex mesh
/// collider over them at `(pos, rot, scale)`, and its world-space vertices.
fn convex_collider(
    name: &str,
    (vertices, indices): (Vec<Vertex>, Vec<u32>),
    (pos, rot, scale): (Vec3, Quat, Vec3),
) -> (Scene, u32, Vec<Vec3>) {
    let locals: Vec<Vec3> = vertices.iter().map(|v| Vec3::from(v.position)).collect();
    let (min, max) = locals.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), &p| {
        (lo.min(p), hi.max(p))
    });
    let mut scene = Scene::new();
    let id = scene.add_entity("Hull".to_string());
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.rotation, t.scale) = (pos, rot, scale);
    drop(t);
    scene.world.set_static(id, true);
    let mesh = MeshComponent::new(name, vertices, indices);
    scene.world.set_mesh(id, Some(mesh));
    let collider = ColliderComponent::from_mesh_bounds(true, min, max);
    scene.world.set_collider(id, Some(collider));
    let world = locals.iter().map(|&p| pos + rot * (p * scale)).collect();
    (scene, id, world)
}
