//! A mesh modelled in world coordinates (its entity origin far from its geometry,
//! like any architecture model) is placed by its world bounds' centre, Unity's
//! `Renderer.bounds.center`: the transparent sort and the light-probe
//! sample both read it, never the origin.

mod probes;
mod transparent;

use glam::Vec3;
use rusty::components::mesh::Vertex;
use rusty::scene::{MeshComponent, Scene};

/// Add an entity at `origin` whose mesh is authored at world position `at` (vertices
/// shifted by `at - origin`).
pub(super) fn world_modelled(
    scene: &mut Scene,
    name: &str,
    (origin, at): (Vec3, Vec3),
    (mut vertices, indices): (Vec<Vertex>, Vec<u32>),
) -> u32 {
    let shift = at - origin;
    for v in &mut vertices {
        v.position = (Vec3::from(v.position) + shift).into();
    }
    let id = scene.add_entity(name.to_string());
    scene.world.transform_mut(id).unwrap().position = origin;
    let mesh = MeshComponent::new(name, vertices, indices);
    scene.world.set_mesh(id, Some(mesh));
    id
}

pub(super) fn centre_rgb(path: &std::path::Path) -> [u8; 3] {
    let img = image::open(path).expect("png").to_rgb8();
    img.get_pixel(img.width() / 2, img.height() / 2).0
}
