//! Flat normals for a mesh that brings none, as the glTF spec requires (it also says to
//! ignore provided tangents). Defaulting to straight up would light every face as facing
//! the sky. [`flat_shaded`] splits shared vertices: a flat normal belongs to a face.

use super::mesh_data::Vertex;
use super::tangents::Missing;
use glam::Vec3;

/// A degenerate triangle gets `+Y`: it covers no pixels, so it only has to be finite.
pub fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let [a, b, c] = [a, b, c].map(Vec3::from_array);
    (b - a)
        .cross(c - a)
        .try_normalize()
        .unwrap_or(Vec3::Y)
        .to_array()
}

/// A triangle naming a vertex past the end is dropped, not read out of bounds.
pub fn flat_shaded(
    vertices: &[Vertex],
    indices: &[u32],
    missing: Missing,
) -> (Vec<Vertex>, Vec<u32>) {
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
        out.extend(corners.into_iter().map(|v| Vertex { normal, ..v }));
    }
    let indices: Vec<u32> = (0..out.len() as u32).collect();
    let positions: Vec<[f32; 3]> = out.iter().map(|v| v.position).collect();
    let normals: Vec<[f32; 3]> = out.iter().map(|v| v.normal).collect();
    let uvs: Vec<[f32; 2]> = out.iter().map(|v| v.tex_coords).collect();
    let tangents = super::tangents::fill(missing, &positions, &normals, &uvs, &indices);
    for (v, t) in out.iter_mut().zip(tangents) {
        v.tangent = t;
    }
    (out, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex(position: [f32; 3]) -> Vertex {
        Vertex::new(position.into(), glam::Vec3::ZERO, [0.0; 2])
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
        let vertices = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, -1.0],
        ]
        .map(vertex);
        let (out, indices) = flat_shaded(&vertices, &[0, 1, 2, 0, 1, 3], Missing::Generate);
        assert_eq!(indices, vec![0, 1, 2, 3, 4, 5]);
        let normals: Vec<[f32; 3]> = out.iter().map(|v| v.normal).collect();
        assert_eq!(normals[..3], [[0.0, 0.0, 1.0]; 3]);
        assert_eq!(normals[3..], [[0.0, 1.0, 0.0]; 3]);
        assert_eq!(out[4].position, [1.0, 0.0, 0.0]);
        for v in &out {
            let t = Vec3::from_slice(&v.tangent[..3]);
            assert!(t.dot(Vec3::from(v.normal)).abs() < 1e-5, "{v:?}");
        }
    }

    #[test]
    fn a_triangle_past_the_end_is_dropped() {
        let vertices = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]].map(vertex);
        let (out, indices) = flat_shaded(&vertices, &[0, 1, 2, 0, 1, 9], Missing::Generate);
        assert_eq!((out.len(), indices.len()), (3, 3));
    }
}
