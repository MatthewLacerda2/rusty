use super::*;
use std::f32::consts::TAU;

fn xyz(t: [f32; 4]) -> Vec3 {
    Vec3::new(t[0], t[1], t[2])
}

/// A unit-square quad in the XY plane facing +Z, UVs mapped by `uv(x, y)`.
fn quad(uv: fn(f32, f32) -> [f32; 2]) -> Vec<[f32; 4]> {
    let positions = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let uvs: Vec<[f32; 2]> = positions.iter().map(|p| uv(p[0], p[1])).collect();
    generate_tangents(&positions, &[[0.0, 0.0, 1.0]; 4], &uvs, &[0, 1, 2, 0, 2, 3])
}

/// An open, smooth-shaded cylinder tube (radius 1, height 1) whose U wraps the
/// circumference and V runs up the axis — the reference mesh: its exact MikkTSpace
/// tangent is the circumferential direction dP/dU = (-sin θ, 0, -cos θ), right-handed.
fn cylinder(segments: u32) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>) {
    let (mut pos, mut nrm, mut uv, mut idx) = (vec![], vec![], vec![], vec![]);
    for i in 0..=segments {
        let u = i as f32 / segments as f32;
        let (s, c) = (u * TAU).sin_cos();
        for y in [0.0, 1.0] {
            pos.push([c, y, -s]);
            nrm.push([c, 0.0, -s]);
            uv.push([u, y]);
        }
    }
    for i in 0..segments {
        let (a, b) = (2 * i, 2 * i + 2);
        // Outward-facing (counter-clockwise seen from outside).
        idx.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
    }
    (pos, nrm, uv, idx)
}

#[test]
fn flat_quad_tangent_follows_u_axis_right_handed() {
    for t in quad(|x, y| [x, y]) {
        assert!(
            xyz(t).abs_diff_eq(Vec3::X, 1e-5),
            "tangent must be +X: {t:?}"
        );
        assert_eq!(t[3], 1.0, "U→+X, V→+Y on a +Z face is right-handed");
    }
}

#[test]
fn mirrored_u_flips_tangent_and_handedness() {
    // U runs along -X: the tangent follows it and the basis becomes left-handed.
    for t in quad(|x, y| [1.0 - x, y]) {
        assert!(
            xyz(t).abs_diff_eq(-Vec3::X, 1e-5),
            "tangent must be -X: {t:?}"
        );
        assert_eq!(t[3], -1.0, "a mirrored UV is encoded as w = -1");
    }
}

#[test]
fn cylinder_matches_its_analytic_reference_tangent() {
    let (pos, nrm, uv, idx) = cylinder(32);
    let tangents = generate_tangents(&pos, &nrm, &uv, &idx);
    for (i, t) in tangents.iter().enumerate() {
        let (s, c) = (uv[i][0] * TAU).sin_cos();
        let expected = Vec3::new(-s, 0.0, -c);
        assert!(
            xyz(*t).abs_diff_eq(expected, 1e-3),
            "vertex {i}: {t:?} vs {expected}"
        );
        assert_eq!(t[3], 1.0, "vertex {i} must be right-handed");
    }
}

#[test]
fn every_tangent_is_unit_orthogonal_to_its_normal_with_signed_w() {
    // Normals tilted off the surface, so orthogonalization actually has work to do.
    let (pos, mut nrm, uv, idx) = cylinder(12);
    for n in &mut nrm {
        *n = (Vec3::from_array(*n) + Vec3::new(0.3, 0.5, -0.2)).to_array();
    }
    for (t, n) in generate_tangents(&pos, &nrm, &uv, &idx).iter().zip(&nrm) {
        let (v, n) = (xyz(*t), Vec3::from_array(*n).normalize());
        assert!(
            (v.length() - 1.0).abs() < 1e-4,
            "tangent must be unit: {t:?}"
        );
        assert!(
            v.dot(n).abs() < 1e-4,
            "tangent must be orthogonal to N: {t:?}"
        );
        assert!(
            t[3] == 1.0 || t[3] == -1.0,
            "w is a ±1 handedness sign: {t:?}"
        );
    }
}

#[test]
fn degenerate_uvs_still_yield_a_finite_orthogonal_tangent() {
    let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let normals = [[0.0, 0.0, 1.0]; 3];
    let uvs = [[0.0, 0.0]; 3]; // zero-area UVs
    for t in generate_tangents(&positions, &normals, &uvs, &[0, 1, 2]) {
        let v = xyz(t);
        assert!(
            (v.length() - 1.0).abs() < 1e-4,
            "tangent must be unit: {t:?}"
        );
        assert!(
            v.dot(Vec3::Z).abs() < 1e-4,
            "tangent must be orthogonal to N"
        );
        assert!(t[3].abs() == 1.0, "handedness is ±1");
    }
}

#[test]
fn unreferenced_vertex_gets_the_fallback_tangent() {
    // Vertex 3 belongs to no triangle: MikkTSpace never visits it.
    let positions = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [5.0, 5.0, 5.0],
    ];
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]];
    let t = generate_tangents(&positions, &[[0.0, 0.0, 1.0]; 4], &uvs, &[0, 1, 2]);
    assert_eq!(t[3], fallback(Vec3::Z));
    assert!(xyz(t[3]).dot(Vec3::Z).abs() < 1e-6);
}
