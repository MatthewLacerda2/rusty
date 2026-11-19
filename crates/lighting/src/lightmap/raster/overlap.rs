//! Triangles that overlap in UV (#2310). The lightmap's own UVs are packed apart, so
//! its walk keeps each texel's first triangle and never asks; the mesh-map baker
//! reads a model's UV0, where a panel and the wall behind it, or a box's front and
//! hidden back, can share texels, so it asks the walk for every claimant
//! ([`rasterize_overlaps`]) and picks one itself (`mesh_maps::overlap`).

use glam::Vec2;

use super::{walk, Surface, TexelPoint};

/// How deep, in texels, two triangles must reach into each other to overlap: two
/// that only share an edge (or touch within float error) never do.
const DEPTH: f32 = 1e-4;

/// [`rasterize_surface`](super::rasterize_surface)'s points, and every other
/// triangle's point on a texel whose owner it overlaps in UV: (the owner's place
/// in the points, the claimant's point), in walk order. A layout with no overlap
/// gets the same points and no claimant.
pub(crate) fn rasterize_overlaps(
    mesh: &Surface,
    width: u32,
    height: u32,
) -> (Vec<TexelPoint>, Vec<(u32, TexelPoint)>) {
    let mut others = Vec::new();
    let points = walk(mesh, width, height, Some(&mut others));
    (points, others)
}

/// The UVs, scaled by `scale`, of the triangle starting at index `first`.
pub(super) fn uv_of(mesh: &Surface, first: usize, scale: Vec2) -> [Vec2; 3] {
    [0, 1, 2].map(|k| mesh.uvs[mesh.indices[first + k] as usize] * scale)
}

/// Whether triangles `a` and `b` share area (deeper than [`DEPTH`]): no edge of
/// either separates them (the separating-axis test, which is exact for two convex
/// shapes).
pub(super) fn in_uv(a: [Vec2; 3], b: [Vec2; 3]) -> bool {
    !separates(&a, &b) && !separates(&b, &a)
}

/// Whether an edge of `p` has `p` and `q` on its two sides, within [`DEPTH`].
fn separates(p: &[Vec2; 3], q: &[Vec2; 3]) -> bool {
    (0..3).any(|k| {
        let Some(n) = (p[(k + 1) % 3] - p[k]).perp().try_normalize() else {
            return false;
        };
        let span = |t: &[Vec2; 3]| {
            let d = t.map(|v| v.dot(n));
            (d[0].min(d[1]).min(d[2]), d[0].max(d[1]).max(d[2]))
        };
        let ((plo, phi), (qlo, qhi)) = (span(p), span(q));
        phi < qlo + DEPTH || qhi < plo + DEPTH
    })
}

#[cfg(test)]
mod tests {
    use super::in_uv;
    use glam::Vec2;

    fn tri(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> [Vec2; 3] {
        [a, b, c].map(|(x, y)| Vec2::new(x, y))
    }

    #[test]
    fn two_halves_of_a_quad_touch_but_do_not_overlap() {
        let a = tri((0., 0.), (4., 0.), (4., 4.));
        let b = tri((0., 0.), (4., 4.), (0., 4.));
        assert!(!in_uv(a, b) && !in_uv(b, a));
        let corner = tri((4., 0.), (8., 0.), (8., 4.));
        assert!(!in_uv(a, corner), "sharing a vertex is not overlapping");
    }

    #[test]
    fn identical_nested_and_crossing_triangles_overlap() {
        let a = tri((0., 0.), (4., 0.), (4., 4.));
        assert!(in_uv(a, a), "identical");
        assert!(in_uv(a, tri((3., 1.), (3.5, 1.), (3.5, 2.))), "nested");
        let flipped = tri((0., 4.), (4., 0.), (0., 0.));
        assert!(in_uv(a, flipped), "wound the other way, crossing");
        assert!(!in_uv(a, tri((5., 0.), (9., 0.), (9., 4.))), "apart");
    }
}
