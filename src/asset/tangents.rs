//! Per-vertex tangent generation (Lengyel's method) for normal mapping.
//!
//! A normal map stores perturbations in *tangent space*, so the fragment shader
//! needs a per-vertex tangent basis (T, with the vertex normal N and the derived
//! bitangent B) to rotate them into world space. glTF supplies a `TANGENT` accessor
//! when the exporter wrote one; when it didn't — and for the engine's procedural
//! primitives — we synthesize tangents here from positions + UVs so the same
//! `t_normal` sampling path works for every mesh.
//!
//! The output is the glTF-convention `[x, y, z, w]`: the unit tangent plus a
//! handedness sign `w` (±1) the shader uses to reconstruct the bitangent as
//! `w * cross(N, T)`. Pure math (glam only) — it lives in the asset layer so the
//! importer can call it without crossing into the renderer.

use glam::Vec3;

/// Generate one `[x, y, z, w]` tangent per vertex from triangle UV gradients,
/// Gram-Schmidt-orthogonalized against the vertex normal. `indices` lists triangles
/// (triples); degenerate UVs fall back to an arbitrary tangent orthogonal to N so the
/// basis is always finite. Mirrors the standard Lengyel accumulation.
pub fn generate_tangents(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u32],
) -> Vec<[f32; 4]> {
    let mut tan = vec![Vec3::ZERO; positions.len()];
    let mut bitan = vec![Vec3::ZERO; positions.len()];

    for tri in indices.chunks_exact(3) {
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        accumulate_triangle((i0, i1, i2), positions, uvs, &mut tan, &mut bitan);
    }

    (0..positions.len())
        .map(|i| {
            let n = Vec3::from_array(normals[i]).normalize_or_zero();
            resolve_tangent(n, tan[i], bitan[i])
        })
        .collect()
}

/// Accumulate one triangle's UV-gradient tangent/bitangent into its three vertices.
/// A zero-area UV triangle contributes nothing (left for the per-vertex fallback).
fn accumulate_triangle(
    tri: (usize, usize, usize),
    positions: &[[f32; 3]],
    uvs: &[[f32; 2]],
    tan: &mut [Vec3],
    bitan: &mut [Vec3],
) {
    let (i0, i1, i2) = tri;
    let p0 = Vec3::from_array(positions[i0]);
    let e1 = Vec3::from_array(positions[i1]) - p0;
    let e2 = Vec3::from_array(positions[i2]) - p0;

    let (u0, u1, u2) = (uvs[i0], uvs[i1], uvs[i2]);
    let du1 = u1[0] - u0[0];
    let dv1 = u1[1] - u0[1];
    let du2 = u2[0] - u0[0];
    let dv2 = u2[1] - u0[1];

    let det = du1 * dv2 - du2 * dv1;
    if det.abs() < 1e-12 {
        return;
    }
    let r = 1.0 / det;
    let t = (e1 * dv2 - e2 * dv1) * r;
    let b = (e2 * du1 - e1 * du2) * r;

    for &i in &[i0, i1, i2] {
        tan[i] += t;
        bitan[i] += b;
    }
}

/// Orthonormalize the accumulated tangent against the vertex normal and pack the
/// handedness into `w`. Falls back to an arbitrary orthogonal tangent when the
/// accumulation was degenerate (zero, or parallel to N).
fn resolve_tangent(n: Vec3, accumulated: Vec3, bitan: Vec3) -> [f32; 4] {
    // Gram-Schmidt: subtract the normal-aligned part of the accumulated tangent.
    let t = (accumulated - n * n.dot(accumulated)).normalize_or_zero();
    let t = if t == Vec3::ZERO { orthogonal(n) } else { t };
    // Handedness: +1 unless the bitangent points opposite N x T (a mirrored UV).
    let w = if n.cross(t).dot(bitan) < 0.0 {
        -1.0
    } else {
        1.0
    };
    [t.x, t.y, t.z, w]
}

/// Any unit vector orthogonal to `n` — the tangent fallback for degenerate UVs.
fn orthogonal(n: Vec3) -> Vec3 {
    let axis = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    n.cross(axis).normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_quad_tangent_follows_u_axis() {
        // A quad in the XY plane with U along +X, V along +Y: tangent must be +X.
        let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]];
        let normals = [[0.0, 0.0, 1.0]; 3];
        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];
        let t = generate_tangents(&positions, &normals, &uvs, &[0, 1, 2]);
        assert!(
            (t[0][0] - 1.0).abs() < 1e-4,
            "tangent x should be ~1: {t:?}"
        );
        assert!(t[0][1].abs() < 1e-4 && t[0][2].abs() < 1e-4);
        assert!((t[0][3].abs() - 1.0).abs() < 1e-4, "handedness is ±1");
    }

    #[test]
    fn degenerate_uvs_still_yield_a_finite_orthogonal_tangent() {
        let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let normals = [[0.0, 0.0, 1.0]; 3];
        let uvs = [[0.0, 0.0]; 3]; // zero-area UVs
        let t = generate_tangents(&positions, &normals, &uvs, &[0, 1, 2]);
        let v = Vec3::new(t[0][0], t[0][1], t[0][2]);
        assert!((v.length() - 1.0).abs() < 1e-4, "tangent must be unit");
        assert!(
            v.dot(Vec3::Z).abs() < 1e-4,
            "tangent must be orthogonal to N"
        );
    }
}
