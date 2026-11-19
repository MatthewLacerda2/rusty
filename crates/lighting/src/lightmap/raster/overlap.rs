//! Triangles that overlap in UV (#2310). The lightmap's own UVs are packed apart, so
//! its walk keeps each texel's first triangle and never asks; the mesh-map baker
//! reads a model's UV0, where a panel and the wall behind it, or the two mirrored
//! halves of a symmetric model, share texels, so it asks the walk for every
//! claimant ([`rasterize_claims`]) and picks one itself (`mesh_maps::overlap`).

use glam::Vec2;

use super::{walk, Surface, TexelPoint};

/// How deep, in texels, two triangles must reach into each other to overlap, near
/// the origin: two that only share an edge (or touch within float error) never
/// do. Far from it the depth grows with the coordinates ([`depth`]).
const DEPTH: f32 = 1e-4;

/// One later triangle's point on a texel another already owns, whose triangle
/// overlaps the owner's in UV ([`in_uv`]), as [`walk`] hands it to a
/// claim sink.
pub(crate) struct Claim<'a> {
    /// The texel's place in the walk's points.
    pub slot: u32,
    /// The first index of the triangle that owns the texel in the walk.
    pub owner: u32,
    /// The claimant's own point on the texel.
    pub point: TexelPoint,
    /// The texel's point as it stands: the owner's, unless the sink replaced it.
    pub kept: &'a mut TexelPoint,
}

/// Who holds each texel during a [`walk`]: a flag each when nobody asks for
/// claims (the lightmap, which pays a byte a texel), or each texel's place in the
/// points and, per point, its owner's first index (the mesh-map baker).
pub(super) enum Owners {
    Flags(Vec<bool>),
    Slots { slot: Vec<u32>, first: Vec<u32> },
}

/// A texel's holder in [`Owners`].
pub(super) enum Owner {
    Free,
    Taken,
    /// Its place in the points and its owner's first index.
    At(u32, u32),
}

impl Owners {
    pub(super) fn new(texels: usize, claims: bool) -> Self {
        match claims {
            false => Owners::Flags(vec![false; texels]),
            true => Owners::Slots {
                slot: vec![u32::MAX; texels],
                first: Vec::new(),
            },
        }
    }

    pub(super) fn of(&self, index: usize) -> Owner {
        match self {
            Owners::Flags(f) if f[index] => Owner::Taken,
            Owners::Slots { slot, first } if slot[index] != u32::MAX => {
                let s = slot[index];
                Owner::At(s, first[s as usize])
            }
            _ => Owner::Free,
        }
    }

    /// Give texel `index` to point `at` of triangle `first`.
    pub(super) fn take(&mut self, index: usize, at: usize, first: u32) {
        match self {
            Owners::Flags(f) => f[index] = true,
            Owners::Slots {
                slot,
                first: firsts,
            } => {
                slot[index] = at as u32;
                firsts.push(first);
            }
        }
    }
}

/// [`rasterize_surface`](super::rasterize_surface)'s points, handing `sink` every
/// other triangle's point on a texel whose owner it overlaps in UV, in walk order.
/// A layout with no overlap gets the same points and never calls `sink`.
pub(crate) fn rasterize_claims(
    mesh: &Surface,
    width: u32,
    height: u32,
    sink: &mut dyn FnMut(Claim),
) -> Vec<TexelPoint> {
    walk(mesh, width, height, Some(sink))
}

/// The last (triangle, owner) pair [`walk`] asked [`in_uv`] about, and the answer:
/// a triangle's texels over one owner come in a run, so the walk asks once a run.
#[derive(Default)]
pub(super) struct Asked(Option<(usize, u32, bool)>);

impl Asked {
    /// Whether triangle `t` (its first index and texel-space UVs) overlaps, in UV,
    /// the triangle starting at index `owner`.
    pub(super) fn overlaps(
        &mut self,
        mesh: &Surface,
        t: (usize, [Vec2; 3]),
        owner: u32,
        scale: Vec2,
    ) -> bool {
        match self.0 {
            Some((a, b, v)) if (a, b) == (t.0, owner) => v,
            _ => {
                let v = in_uv(t.1, uv_of(mesh, owner as usize, scale));
                self.0 = Some((t.0, owner, v));
                v
            }
        }
    }
}

/// The UVs, scaled by `scale`, of the triangle starting at index `first`.
fn uv_of(mesh: &Surface, first: usize, scale: Vec2) -> [Vec2; 3] {
    [0, 1, 2].map(|k| mesh.uvs[mesh.indices[first + k] as usize] * scale)
}

/// Whether triangles `a` and `b` share area (deeper than [`depth`]): no edge of
/// either separates them (the separating-axis test, which is exact for two convex
/// shapes). Two that share an edge (two UV vertices) with their third vertices on
/// either side of it never do, decided by sign alone, as every neighbour in a
/// mesh is. The rest runs about `a`'s first vertex, so the dot products carry the
/// triangles' size, not where in a large map they sit, and the depth grows with
/// the coordinates' own rounding (a texel's UV scaled to 8192 is good to 1e-3).
pub(super) fn in_uv(a: [Vec2; 3], b: [Vec2; 3]) -> bool {
    if across_a_shared_edge(&a, &b) {
        return false;
    }
    let reach = a
        .iter()
        .chain(&b)
        .fold(0.0f32, |m, v| m.max(v.abs().max_element()));
    let depth = depth(reach);
    let o = a[0];
    let (a, b) = (a.map(|v| v - o), b.map(|v| v - o));
    !separates(&a, &b, depth) && !separates(&b, &a, depth)
}

/// The overlap depth for coordinates up to `reach` texels from the image's corner:
/// each is good to half an f32 unit in the last place at that size, and a
/// projection of the six to a few of them.
fn depth(reach: f32) -> f32 {
    DEPTH.max(16.0 * f32::EPSILON * reach)
}

/// Whether `a` and `b` share exactly two vertices and lie on the two sides of that
/// edge.
fn across_a_shared_edge(a: &[Vec2; 3], b: &[Vec2; 3]) -> bool {
    let mut shared = [Vec2::ZERO; 3];
    let mut n = 0;
    for v in a.iter().filter(|v| b.contains(v)) {
        shared[n] = *v;
        n += 1;
    }
    if n != 2 {
        return false;
    }
    let (p, q) = (shared[0], shared[1]);
    let lone = |t: &[Vec2; 3]| *t.iter().find(|v| **v != p && **v != q).unwrap_or(&p);
    let side = |v: Vec2| (q - p).perp_dot(v - p);
    side(lone(a)) * side(lone(b)) < 0.0
}

/// Whether an edge of `p` has `p` and `q` on its two sides, within `depth`.
fn separates(p: &[Vec2; 3], q: &[Vec2; 3], depth: f32) -> bool {
    (0..3).any(|k| {
        let Some(n) = (p[(k + 1) % 3] - p[k]).perp().try_normalize() else {
            return false;
        };
        let span = |t: &[Vec2; 3]| {
            let d = t.map(|v| v.dot(n));
            (d[0].min(d[1]).min(d[2]), d[0].max(d[1]).max(d[2]))
        };
        let ((plo, phi), (qlo, qhi)) = (span(p), span(q));
        phi < qlo + depth || qhi < plo + depth
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

    /// A fixed stream of numbers in 0..1.
    fn draws() -> impl FnMut() -> f32 {
        let mut seed = 0x2310u64;
        move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 40) as f32 / (1u64 << 24) as f32
        }
    }

    #[test]
    fn neighbours_never_overlap_however_far_into_an_8k_map() {
        let mut next = draws();
        for size in [1024.0, 4096.0, 8192.0f32] {
            for k in 0..2000 {
                let p = Vec2::new(next(), next()) * size;
                let step = match k % 2 {
                    0 => Vec2::new(next(), next()) * 8.0 + 0.5,
                    _ => Vec2::new(next() * 8.0 + 0.5, 0.0),
                };
                let q = p + step;
                let n = step.perp() * (0.1 + next());
                let mid = (p + q) * 0.5;
                let (a, b) = ([p, q, mid + n], [q, p, mid - n]);
                assert!(!in_uv(a, b), "sharing an edge at {size}: {a:?} {b:?}");
                // A vertex on the other's edge (a T-junction), sharing one vertex.
                let t = [q, mid, mid - n];
                assert!(!in_uv(a, t), "a T-junction at {size}: {a:?} {t:?}");
            }
        }
    }

    #[test]
    fn a_triangle_folded_back_over_its_shared_edge_still_overlaps() {
        let a = tri((0., 0.), (4., 0.), (2., 4.));
        assert!(in_uv(a, tri((4., 0.), (0., 0.), (2., 2.))), "same side");
        let far = a.map(|v| v + Vec2::splat(8000.0));
        assert!(in_uv(far, far), "identical, far into the map");
    }
}
