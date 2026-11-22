use glam::{Vec2, Vec3};

use super::UvTri;

/// A point on a triangle's edge sits on whatever surface meets the mesh there, so rays from it leak
/// or start on its back: texels claimed by overlap are traced this far in (texels).
pub(in super::super) const INSET: f32 = 0.125;

/// For overlaps too thin for [`INSET`]: the share of the triangle's local width (two nearest edge
/// distances, summed) kept off every edge.
const THIN_SHARE: f32 = 0.25;

/// The most vertices a triangle clipped by seven half-planes can have.
const MAX_VERTS: usize = 10;

#[derive(Clone, Copy, Default)]
struct Vert {
    uv: Vec2,
    w: Vec3,
}

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

    /// Weights are carried through the clip, never recovered by dividing by the triangle's (maybe
    /// tiny) area.
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
