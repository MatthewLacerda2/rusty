//! The built-in primitives' lightmap UVs (#438): a Box and a Plane are lightmappable
//! out of the box, with every lightmap UV inside [0, 1] and no two faces sharing one.

use super::primitives::{generate_box, generate_plane, generate_sphere};

/// The lightmap-UV bounding rectangle of each quad (four vertices per face).
fn face_rects(vertices: &[super::Vertex]) -> Vec<([f32; 2], [f32; 2])> {
    vertices
        .chunks(4)
        .map(|quad| {
            let mut min = [f32::MAX; 2];
            let mut max = [f32::MIN; 2];
            for v in quad {
                for a in 0..2 {
                    min[a] = min[a].min(v.lightmap_uv[a]);
                    max[a] = max[a].max(v.lightmap_uv[a]);
                }
            }
            (min, max)
        })
        .collect()
}

#[test]
fn box_faces_get_disjoint_lightmap_cells() {
    let (vertices, _) = generate_box(2.0, 1.0, 3.0);
    let rects = face_rects(&vertices);
    assert_eq!(rects.len(), 6);
    for (i, (min, max)) in rects.iter().enumerate() {
        assert!(min.iter().chain(max).all(|c| (0.0..=1.0).contains(c)));
        assert!(
            max[0] > min[0] && max[1] > min[1],
            "face {i} has lightmap area"
        );
        for (other_min, other_max) in &rects[i + 1..] {
            let apart = max[0] <= other_min[0]
                || other_max[0] <= min[0]
                || max[1] <= other_min[1]
                || other_max[1] <= min[1];
            assert!(apart, "face {i}'s cell overlaps another");
        }
    }
}

#[test]
fn plane_lightmap_uv_is_its_texture_uv() {
    let (vertices, _) = generate_plane(4.0, 4.0);
    assert!(vertices.iter().all(|v| v.lightmap_uv == v.tex_coords));
}

#[test]
fn sphere_has_no_lightmap_uv() {
    let (vertices, _) = generate_sphere(1.0, 8, 8);
    assert!(vertices.iter().all(|v| v.lightmap_uv == [0.0, 0.0]));
}
