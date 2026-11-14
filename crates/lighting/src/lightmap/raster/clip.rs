//! Where a texel a triangle only overlaps is traced (#1687): the triangle clipped
//! to the texel square, then to [`INSET`] off its edges, all on the stack.

use glam::{Vec2, Vec3};

use super::UvTri;

/// How far in from its triangle's edges a texel claimed by overlap is traced, in
/// texels, when the overlap is deep enough (#1687). A point on an edge sits on
/// whatever surface meets the mesh there (a wall a floor stops at, the other face
/// of a crease), so rays from it leak through that surface or start on its back.
pub(in super::super) const INSET: f32 = 0.125;

/// Where the overlap is too thin for [`INSET`], the share of the triangle's local
/// width (the distances to its two nearest edges, summed) a traced point must keep
/// off every edge. A sliver's overlap centroid sits on its midline, half the width.
const THIN_SHARE: f32 = 0.25;

/// The most vertices a triangle clipped by seven half-planes can have.
const MAX_VERTS: usize = 10;

/// A point in texel space and its barycentric weights in the triangle it lies in.
#[derive(Clone, Copy, Default)]
struct Vert {
    uv: Vec2,
    w: Vec3,
}

/// A convex polygon of at most [`MAX_VERTS`] vertices, on the stack.
#[derive(Clone, Copy)]
struct Poly {
    v: [Vert; MAX_VERTS],
    n: usize,
}

impl Poly {
    fn verts(&self) -> &[Vert] {
        &self.v[..self.n]
    }
}

/// Triangle `t`'s corners with their weights, and the distance from each corner to
/// the opposite edge (texels): a point's distance to edge `i` is `w[i] * height[i]`.
pub(super) struct Clipper {
    tri: Poly,
    height: Vec3,
}

impl Clipper {
    pub(super) fn new(t: &UvTri) -> Self {
        let mut tri = Poly {
            v: [Vert::default(); MAX_VERTS],
            n: 3,
        };
        for (k, w) in [Vec3::X, Vec3::Y, Vec3::Z].into_iter().enumerate() {
            tri.v[k] = Vert { uv: t.uv[k], w };
        }
        let side = |i: usize, j: usize| t.uv[i].distance(t.uv[j]);
        let sides = Vec3::new(side(1, 2), side(2, 0), side(0, 1));
        Self {
            tri,
            height: t.area.abs() / sides,
        }
    }

    /// The barycentric weights to trace the texel whose lower corner is `lo` at, or
    /// `None` when the triangle leaves it to dilation. The triangle is clipped to the
    /// texel; that overlap clipped again to [`INSET`] off every edge gives the
    /// centroid when anything is left. Otherwise (a sliver, a tip) the overlap's own
    /// centroid is taken when it keeps `min(INSET, THIN_SHARE × local width)` off
    /// every edge, and an overlap shallower than that is left to dilation. The
    /// weights are carried through the clip, never recovered by dividing by the
    /// triangle's (maybe tiny) area.
    pub(super) fn trace_weights(&self, lo: Vec2) -> Option<Vec3> {
        let mut poly = self.tri;
        for (axis, bound, sign) in [
            (0, lo.x, 1.0),
            (0, lo.x + 1.0, -1.0),
            (1, lo.y, 1.0),
            (1, lo.y + 1.0, -1.0),
        ] {
            poly = clip(&poly, |v| (v.uv[axis] - bound) * sign)?;
        }
        let mut inset = Some(poly);
        for i in 0..3 {
            let floor = INSET / self.height[i];
            inset = inset.and_then(|p| clip(&p, |v| v.w[i] - floor));
        }
        if let Some(w) = inset.and_then(|p| centroid(&p)) {
            return Some(w);
        }
        let w = centroid(&poly)?;
        let mut d = (w * self.height).to_array();
        d.sort_by(f32::total_cmp);
        (d[0] >= INSET.min(THIN_SHARE * (d[0] + d[1]))).then_some(w)
    }
}

/// The weights of `poly`'s area centroid, or `None` when it encloses no area.
fn centroid(poly: &Poly) -> Option<Vec3> {
    let v = poly.verts();
    let (mut area, mut sum) = (0.0f32, Vec3::ZERO);
    for pair in v.get(1..)?.windows(2) {
        let a = (pair[0].uv - v[0].uv).perp_dot(pair[1].uv - v[0].uv);
        area += a;
        sum += a * (v[0].w + pair[0].w + pair[1].w) / 3.0;
    }
    // A sliver of float noise where the shapes only touch is no overlap.
    (area.abs() > 1e-9).then(|| sum / area)
}

/// `poly` with the part where `inside` is negative cut away (one Sutherland–Hodgman
/// step), each new vertex's weights interpolated with its position; `None` when
/// fewer than three vertices are left.
fn clip(poly: &Poly, inside: impl Fn(&Vert) -> f32) -> Option<Poly> {
    let mut out = Poly {
        v: [Vert::default(); MAX_VERTS],
        n: 0,
    };
    let v = poly.verts();
    for (k, a) in v.iter().enumerate() {
        let b = &v[(k + 1) % v.len()];
        let (da, db) = (inside(a), inside(b));
        if da >= 0.0 && out.n < MAX_VERTS {
            out.v[out.n] = *a;
            out.n += 1;
        }
        if (da >= 0.0) != (db >= 0.0) && out.n < MAX_VERTS {
            let s = da / (da - db);
            out.v[out.n] = Vert {
                uv: a.uv.lerp(b.uv, s),
                w: a.w.lerp(b.w, s),
            };
            out.n += 1;
        }
    }
    (out.n >= 3).then_some(out)
}
