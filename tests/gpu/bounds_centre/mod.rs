//! A mesh modelled in world coordinates (its entity origin far from its geometry,
//! like any architecture model) is placed by its world bounds' centre, Unity's
//! `Renderer.bounds.center`: the transparent sort (#1993) and the light-probe
//! sample (#1994) both read it, never the origin.
//!
//! Colours are light-independent where they can be: zero ambient and no lights.
//! Each test skips (passes without asserting) when no GPU / software adapter exists.

mod probes;
mod transparent;

use glam::Vec3;
use rusty::components::mesh::Vertex;
use rusty::scene::{MeshComponent, Scene};

/// Add an entity standing at `origin` whose mesh `(vertices, indices)` is authored at
/// world position `at`: its vertices are shifted by `at - origin`, so the geometry
/// sits at `at` while the entity's origin is elsewhere.
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

/// The RGB of `path`'s centre pixel.
pub(super) fn centre_rgb(path: &std::path::Path) -> [u8; 3] {
    let img = image::open(path).expect("png").to_rgb8();
    img.get_pixel(img.width() / 2, img.height() / 2).0
}
