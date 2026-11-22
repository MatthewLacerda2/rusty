use glam::{Quat, Vec3};

use rusty_components::mesh::Vertex;
use rusty_components::{ColliderComponent, MeshComponent};
use rusty_scene::create::{primitive_geometry, Primitive};
use rusty_scene::Scene;

/// A test hook for `script`'s point-query tests: the off-centre cylinder as a convex
/// collider, turned and scaled, and its world-space vertices.
#[doc(hidden)]
pub fn turned_hull() -> (Scene, u32, Vec<Vec3>) {
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

#[cfg(test)]
pub(super) fn flat_hull() -> (Scene, u32) {
    let (vertices, indices) = primitive_geometry(Primitive::Plane).expect("a mesh");
    let pose = (Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
    let geometry = (vertices.to_vec(), indices.to_vec());
    let (scene, id, _) = convex_collider("Plane", geometry, pose);
    (scene, id)
}

/// The Cylinder primitive moved off its origin, so its local box is not centred. Its caps
/// are rings of coplanar points, the classic quickhull degeneracy.
pub(crate) fn off_centre_cylinder() -> (Vec<Vertex>, Vec<u32>) {
    let (vertices, indices) = primitive_geometry(Primitive::Cylinder).expect("a mesh");
    let (mut vertices, indices) = (vertices.to_vec(), indices.to_vec());
    let offset = Vec3::new(0.3, 0.2, -0.1);
    vertices
        .iter_mut()
        .for_each(|v| v.position = (Vec3::from(v.position) + offset).to_array());
    (vertices, indices)
}

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
    let mut t = scene
        .world
        .transform_mut(id)
        .expect("an entity just added has a Transform");
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
