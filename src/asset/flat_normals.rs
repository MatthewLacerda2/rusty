//! Flat normals for a mesh that brings none (#1217).
//!
//! The glTF 2.0 spec: "When normals are not specified, client implementations
//! MUST calculate flat normals and the provided tangents (if present) MUST be
//! ignored." Every importer takes its missing normals from here instead of
//! defaulting to straight up, which lights every face as if it faced the sky and
//! breaks anything that pushes vertices along their normals (an outline shell).
//!
//! A flat normal belongs to a face, not a vertex, so a vertex two faces share is
//! split: [`flat_shaded`] gives every triangle corner its own copy.

use super::mesh_data::MeshVertex;
use glam::Vec3;

/// The unit normal of the triangle `a, b, c`, facing the side it winds
/// counter-clockwise on (glTF's front face). A degenerate triangle has no plane;
/// it covers no pixels, so it gets `+Y`, which only has to be finite.
pub fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let [a, b, c] = [a, b, c].map(Vec3::from_array);
    (b - a)
        .cross(c - a)
        .try_normalize()
        .unwrap_or(Vec3::Y)
        .to_array()
}

/// `vertices` drawn by the triangle list `indices`, unwelded so each corner is
/// its own vertex carrying its triangle's [`face_normal`] and a tangent generated
/// against it (a source tangent is dropped, as the spec says). Returns the new
/// vertices and their indices, `0..n`. A triangle naming a vertex past the end is
/// dropped rather than read out of bounds.
pub fn flat_shaded(vertices: &[MeshVertex], indices: &[u32]) -> (Vec<MeshVertex>, Vec<u32>) {
    let mut out = Vec::with_capacity(indices.len());
    for tri in indices.chunks_exact(3) {
        let Some(corners) = tri
            .iter()
            .map(|&i| vertices.get(i as usize).copied())
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let normal = face_normal(
            corners[0].position,
            corners[1].position,
            corners[2].position,
        );
        out.extend(corners.into_iter().map(|v| MeshVertex { normal, ..v }));
    }
    let indices: Vec<u32> = (0..out.len() as u32).collect();
    let positions: Vec<[f32; 3]> = out.iter().map(|v| v.position).collect();
    let normals: Vec<[f32; 3]> = out.iter().map(|v| v.normal).collect();
    let uvs: Vec<[f32; 2]> = out.iter().map(|v| v.tex_coords).collect();
    let tangents = super::tangents::generate_tangents(&positions, &normals, &uvs, &indices);
    for (v, t) in out.iter_mut().zip(tangents) {
        v.tangent = t;
    }
    (out, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex(position: [f32; 3]) -> MeshVertex {
        MeshVertex::new(position, [0.0; 3], [0.0; 2])
    }

    #[test]
    fn a_counter_clockwise_triangle_faces_the_viewer() {
        let n = face_normal([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert_eq!(n, [0.0, 0.0, 1.0]);
        let back = face_normal([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]);
        assert_eq!(back, [0.0, 0.0, -1.0]);
    }

    #[test]
    fn a_degenerate_triangle_gets_a_finite_normal() {
        let p = [1.0, 2.0, 3.0];
        assert_eq!(face_normal(p, p, p), [0.0, 1.0, 0.0]);
    }

    #[test]
    fn a_shared_vertex_splits_so_each_face_keeps_its_normal() {
        // Two triangles folded along the shared edge 0-1, one in z=0, one in y=0.
        let vertices = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, -1.0],
        ]
        .map(vertex);
        let (out, indices) = flat_shaded(&vertices, &[0, 1, 2, 0, 1, 3]);
        assert_eq!(indices, vec![0, 1, 2, 3, 4, 5]);
        let normals: Vec<[f32; 3]> = out.iter().map(|v| v.normal).collect();
        assert_eq!(normals[..3], [[0.0, 0.0, 1.0]; 3]);
        assert_eq!(normals[3..], [[0.0, 1.0, 0.0]; 3]);
        assert_eq!(out[4].position, [1.0, 0.0, 0.0]);
        // The tangent is regenerated against the flat normal, so it lies in the face.
        for v in &out {
            let t = Vec3::from_slice(&v.tangent[..3]);
            assert!(t.dot(Vec3::from(v.normal)).abs() < 1e-5, "{v:?}");
        }
    }

    #[test]
    fn a_triangle_past_the_end_is_dropped() {
        let vertices = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]].map(vertex);
        let (out, indices) = flat_shaded(&vertices, &[0, 1, 2, 0, 1, 9]);
        assert_eq!((out.len(), indices.len()), (3, 3));
    }
}
