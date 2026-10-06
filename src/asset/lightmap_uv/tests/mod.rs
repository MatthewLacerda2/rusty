//! Generate Lightmap UVs (#831): every triangle gets UV area inside `[0, 1]²`, no two
//! triangles overlap, charts sit at least the pack margin apart, and the same input
//! always unwraps the same way.

use glam::{Vec2, Vec3};

use super::overlap::interiors_overlap;
use super::{LightmapUvSettings, Unwrap};

pub(super) fn on() -> LightmapUvSettings {
    LightmapUvSettings {
        generate: true,
        ..Default::default()
    }
}

/// A welded box: 8 corners, 12 triangles.
pub(super) fn cube(size: f32) -> (Vec<[f32; 3]>, Vec<u32>) {
    let s = size / 2.0;
    let positions = (0..8)
        .map(|i| [i & 1, (i >> 1) & 1, (i >> 2) & 1].map(|b| if b == 1 { s } else { -s }))
        .collect();
    let quads = [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ];
    let indices = quads
        .iter()
        .flat_map(|q| [q[0], q[1], q[2], q[0], q[2], q[3]])
        .collect();
    (positions, indices)
}

/// A ramp spiralling two full turns, rising slowly: every face points nearly up, so
/// flattening it in one piece would lay the second turn over the first.
pub(super) fn spiral_ramp() -> (Vec<[f32; 3]>, Vec<u32>) {
    let steps = 64;
    let mut positions = Vec::new();
    for i in 0..=steps {
        let a = i as f32 / 32.0 * std::f32::consts::TAU;
        let (c, s, y) = (a.cos(), a.sin(), i as f32 * 0.02);
        positions.push([2.0 * c, y, 2.0 * s]);
        positions.push([3.0 * c, y, 3.0 * s]);
    }
    let indices = (0..steps as u32)
        .flat_map(|i| {
            let (a, b, c, d) = (2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 3);
            [a, c, b, b, c, d]
        })
        .collect();
    (positions, indices)
}

fn tris(u: &Unwrap) -> Vec<[Vec2; 3]> {
    u.indices
        .chunks_exact(3)
        .map(|t| [0, 1, 2].map(|k| Vec2::from(u.uvs[t[k] as usize])))
        .collect()
}

/// Inside `[0, 1]²`, no two triangles overlapping, and UV area wherever the triangle
/// has surface (a sphere's pole triangles have none).
pub(super) fn assert_valid(positions: &[[f32; 3]], u: &Unwrap) {
    assert!(u.uvs.iter().flatten().all(|c| (0.0..=1.0).contains(c)));
    let tris = tris(u);
    for (i, a) in tris.iter().enumerate() {
        let [p, q, r] = [0, 1, 2]
            .map(|k| Vec3::from(positions[u.source[u.indices[i * 3 + k] as usize] as usize]));
        if (q - p).cross(r - p).length() > 1e-6 {
            assert!(
                (a[1] - a[0]).perp_dot(a[2] - a[0]).abs() > 0.0,
                "tri {i} has UV area"
            );
        }
        for (j, b) in tris.iter().enumerate().skip(i + 1) {
            assert!(!interiors_overlap(a, b, 1e-6), "tris {i} and {j} overlap");
        }
    }
}

/// Each chart's UV rectangle: triangles sharing an output vertex are one chart.
pub(super) fn chart_rects(u: &Unwrap) -> Vec<(Vec2, Vec2)> {
    let mut chart = (0..u.uvs.len()).collect::<Vec<_>>();
    let root = |c: &mut Vec<usize>, mut i: usize| {
        while c[i] != i {
            i = c[i];
        }
        i
    };
    for t in u.indices.chunks_exact(3) {
        for k in 1..3 {
            let (a, b) = (
                root(&mut chart, t[0] as usize),
                root(&mut chart, t[k] as usize),
            );
            chart[a.max(b)] = a.min(b);
        }
    }
    let mut rects: Vec<(usize, Vec2, Vec2)> = Vec::new();
    for (v, uv) in u.uvs.iter().enumerate() {
        let (r, p) = (root(&mut chart, v), Vec2::from(*uv));
        match rects.iter_mut().find(|x| x.0 == r) {
            Some(x) => (x.1, x.2) = (x.1.min(p), x.2.max(p)),
            None => rects.push((r, p, p)),
        }
    }
    rects.into_iter().map(|(_, lo, hi)| (lo, hi)).collect()
}

mod charts;
mod shapes;
mod survivors;
