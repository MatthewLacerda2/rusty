//! crates/physics/src/shape/fixture.rs — the shape-level test meshes, shared by the
//! conformance suite, the hull tests and the point-query tests.

use glam::Vec3;

pub(crate) use crate::query::convex_fixture::off_centre_cylinder;
use crate::PhysicsWorld;
use rusty_components::mesh::{MeshComponent, Vertex};
use rusty_components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use rusty_scene::authoring::{default_collider, default_rigidbody};
use rusty_scene::Scene;

/// One mesh made of closed boxes, each `(min, max)`, wound outward: a concave
/// shape (an arch, an open box) as one mesh, the way an imported prop arrives.
pub(crate) fn boxes(parts: &[(Vec3, Vec3)]) -> (Vec<[f32; 3]>, Vec<u32>) {
    #[rustfmt::skip]
    const FACES: [u32; 36] = [
        0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 5, 4, 0, 1, 5,
        3, 6, 2, 3, 7, 6, 0, 7, 3, 0, 4, 7, 1, 6, 5, 1, 2, 6,
    ];
    let (mut points, mut indices) = (Vec::new(), Vec::new());
    for &(lo, hi) in parts {
        let base = points.len() as u32;
        for corner in 0..8 {
            let x = if matches!(corner % 4, 1 | 2) {
                hi.x
            } else {
                lo.x
            };
            let y = if matches!(corner % 4, 2 | 3) {
                hi.y
            } else {
                lo.y
            };
            let z = if corner >= 4 { hi.z } else { lo.z };
            points.push([x, y, z]);
        }
        indices.extend(FACES.iter().map(|i| base + i));
    }
    (points, indices)
}

/// An arch 2 m wide and 1 m tall: two 0.4 m legs under a 0.4 m top, 0.5 m deep,
/// its feet at y = 0. Volume 0.6 × 0.4 × 0.5 × 2 + 2 × 0.4 × 0.5 = 0.64 m³.
pub(crate) fn arch() -> (Vec<[f32; 3]>, Vec<u32>) {
    let (d0, d1) = (-0.25, 0.25);
    boxes(&[
        (Vec3::new(-1.0, 0.0, d0), Vec3::new(-0.6, 0.6, d1)),
        (Vec3::new(0.6, 0.0, d0), Vec3::new(1.0, 0.6, d1)),
        (Vec3::new(-1.0, 0.6, d0), Vec3::new(1.0, 1.0, d1)),
    ])
}

/// An open-topped box 2 m across inside, 1 m deep, with 0.2 m walls and floor.
pub(crate) fn bowl() -> (Vec<[f32; 3]>, Vec<u32>) {
    let (w, t, h) = (1.0, 0.2, 1.0);
    boxes(&[
        (Vec3::new(-w - t, 0.0, -w - t), Vec3::new(w + t, t, w + t)),
        (Vec3::new(-w - t, t, -w - t), Vec3::new(-w, h, w + t)),
        (Vec3::new(w, t, -w - t), Vec3::new(w + t, h, w + t)),
        (Vec3::new(-w, t, -w - t), Vec3::new(w, h, -w)),
        (Vec3::new(-w, t, w), Vec3::new(w, h, w + t)),
    ])
}

/// A static floor whose top is at y = 0.
pub(crate) fn floor(scene: &mut Scene) {
    let id = scene.add_entity("Floor".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, -0.5, 0.0);
    scene.world.set_static(id, true);
    let shape = ColliderShape::Box {
        size: Vec3::new(20.0, 1.0, 20.0),
    };
    let collider = ColliderComponent {
        shape,
        ..default_collider()
    };
    scene.world.set_collider(id, Some(collider));
}

/// A body with `mesh` as its concave collider at `at`, kinematic or dynamic.
pub(crate) fn prop(
    scene: &mut Scene,
    mesh: (Vec<[f32; 3]>, Vec<u32>),
    at: Vec3,
    kinematic: bool,
) -> u32 {
    let id = scene.add_entity("Prop".to_string());
    scene.world.transform_mut(id).unwrap().position = at;
    let (lo, hi) = mesh.0.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), &p| {
        (lo.min(p.into()), hi.max(p.into()))
    });
    let vertices = (mesh.0.iter())
        .map(|&p| Vertex::new(Vec3::from_array(p), Vec3::Y, [0.0, 0.0]))
        .collect();
    let component = MeshComponent::new("Asset", vertices, mesh.1);
    scene.world.set_mesh(id, Some(component));
    let collider = ColliderComponent::from_mesh_bounds(false, lo, hi);
    scene.world.set_collider(id, Some(collider));
    let rb = RigidBodyComponent {
        is_kinematic: kinematic,
        mass: 5.0,
        ..default_rigidbody()
    };
    scene.world.set_rigidbody(id, Some(rb));
    id
}

/// Step `world` `n` fixed ticks of 1/60 s.
pub(crate) fn steps(scene: &mut Scene, world: &mut PhysicsWorld, n: usize) {
    for _ in 0..n {
        world.step(scene, 1.0 / 60.0);
    }
}
