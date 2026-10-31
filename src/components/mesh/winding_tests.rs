//! Every built-in primitive winds its triangles counter-clockwise seen from the side
//! its normals face (#1615): the renderer culls a material's back faces by winding,
//! so a primitive wound the other way would draw inside-out.

use glam::Vec3;

use super::primitives::{
    generate_box, generate_capsule, generate_cylinder, generate_plane, generate_plane_grid,
    generate_sphere,
};
use super::Vertex;

/// How many of `indices`' triangles face against their vertex normals.
fn inward(vertices: &[Vertex], indices: &[u32]) -> usize {
    let at = |i: u32| &vertices[i as usize];
    indices
        .chunks_exact(3)
        .filter(|t| {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(at(i).position));
            let face = (b - a).cross(c - a);
            let normals: Vec3 = t.iter().map(|&i| Vec3::from(at(i).normal)).sum();
            // A pole's sliver (two corners a rounding error apart) has no side to face.
            face.length() > 1e-6 && face.dot(normals) <= 0.0
        })
        .count()
}

#[test]
fn every_primitive_faces_its_normals_counter_clockwise() {
    let meshes = [
        ("box", generate_box(1.0, 2.0, 3.0)),
        ("sphere", generate_sphere(0.5, 16, 16)),
        ("cylinder", generate_cylinder(Vec3::NEG_Y, Vec3::Y, 0.5, 16)),
        ("capsule", generate_capsule(0.5, 2.0, 16, 8)),
        ("plane", generate_plane(15.0, 15.0)),
        ("plane grid", generate_plane_grid(4.0, 4.0, 3)),
    ];
    for (name, (vertices, indices)) in meshes {
        assert_eq!(
            inward(&vertices, &indices),
            0,
            "{name} has inward triangles"
        );
    }
}
