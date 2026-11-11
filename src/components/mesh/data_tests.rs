//! Tests for `MeshData` (#2059): what it refuses and the geometry it builds.

use super::*;

fn quad() -> MeshData {
    MeshData {
        positions: vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, -1.0],
            [0.0, 0.0, -1.0],
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        ..Default::default()
    }
}

fn refusal(data: MeshData) -> String {
    data.validate().expect_err("refused")
}

#[test]
fn a_quad_passes_and_its_normals_point_up() {
    let q = quad();
    assert_eq!(q.validate(), Ok(()));
    let (vertices, indices) = q.geometry();
    assert_eq!(indices, q.indices);
    for v in &vertices {
        assert_eq!(v.normal, [0.0, 1.0, 0.0]);
        assert!(v.tangent.iter().all(|t| t.is_finite()));
    }
}

#[test]
fn an_empty_mesh_is_valid_and_builds_nothing() {
    let empty = MeshData::default();
    assert_eq!(empty.validate(), Ok(()));
    assert_eq!(empty.geometry().0.len(), 0);
}

#[test]
fn bad_data_is_refused_with_a_reason() {
    let mut q = quad();
    q.indices.pop();
    assert!(refusal(q).contains("whole number of triangles"));
    let mut q = quad();
    q.indices[4] = 9;
    assert!(refusal(q).contains("index 4 is 9"));
    let mut q = quad();
    q.positions[2][1] = f32::NAN;
    assert!(refusal(q).contains("position 2"));
    let mut q = quad();
    q.uvs = vec![[0.0; 2]];
    assert!(refusal(q).contains("1 uvs for 4 vertices"));
    let mut q = quad();
    q.normals = vec![[0.0, f32::INFINITY, 0.0]; 4];
    assert!(refusal(q).contains("normal 0"));
}

#[test]
fn past_the_vertex_cap_is_refused() {
    let big = MeshData {
        positions: vec![[0.0; 3]; MAX_VERTICES + 1],
        ..Default::default()
    };
    assert!(refusal(big).contains("past the limit"));
    let at_cap = MeshData {
        positions: vec![[0.0; 3]; MAX_VERTICES],
        ..Default::default()
    };
    assert_eq!(at_cap.validate(), Ok(()));
}

#[test]
fn shared_normals_weigh_each_face_by_its_area() {
    // Vertex 0 is shared by a big face in the XZ plane (normal +Y) and a small one
    // in the XY plane (normal +Z): its normal leans to +Y.
    let positions = [
        [0.0, 0.0, 0.0],
        [4.0, 0.0, 0.0],
        [0.0, 0.0, -4.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let n = area_weighted_normals(&positions, &[0, 1, 2, 0, 3, 4]);
    let n0 = Vec3::from_array(n[0]);
    assert!(n0.y > n0.z && n0.z > 0.0, "{n0}");
    assert!((n0.length() - 1.0).abs() < 1e-5);
}

#[test]
fn a_vertex_on_no_face_or_a_degenerate_one_points_up() {
    let n = area_weighted_normals(&[[0.0; 3], [0.0; 3], [0.0; 3], [5.0; 3]], &[0, 1, 2]);
    assert!(n.iter().all(|n| *n == [0.0, 1.0, 0.0]));
}

#[test]
fn given_normals_and_uvs_are_kept() {
    let mut q = quad();
    q.normals = vec![[0.0, 0.0, 2.0]; 4];
    q.uvs = vec![[0.5, 0.25]; 4];
    let (vertices, _) = q.geometry();
    assert_eq!(vertices[3].normal, [0.0, 0.0, 1.0]);
    assert_eq!(vertices[3].tex_coords, [0.5, 0.25]);
}
