use glam::{UVec2, Vec2, Vec3};

use super::{corners, opposite, CLEAR};
#[cfg(all(test, feature = "dev"))]
pub(in crate::occlusion) use cover::without_union;
pub(super) use group::sheets;
use group::tolerance;

mod cover;
mod group;
use crate::occlusion::gather::Face;

/// A texel's edge, metres.
const TEXEL: f32 = 0.25;
/// Per sheet; a larger plane takes coarser texels.
const MAX_TEXELS: usize = 1 << 22;

pub(super) struct Sheet {
    normal: Vec3,
    lo: f32,
    hi: f32,
    face: Face,
    axes: (Vec3, Vec3),
    origin: Vec2,
    texel: f32,
    dims: UVec2,
    sums: Vec<u32>,
    members: Vec<u32>,
}

pub(super) struct Member<'a> {
    pub corners: &'a [Vec3],
    pub normal: Vec3,
    pub mesh: u32,
    pub face: Face,
}

impl Sheet {
    fn of<'a>(members: impl Iterator<Item = &'a Member<'a>> + Clone) -> Option<Self> {
        let first = members.clone().next().expect("a sheet has a member");
        let normal = first.normal;
        let axes = axes(normal);
        let flat = |p: Vec3| Vec2::new(axes.0.dot(p), axes.1.dot(p));
        let all = members.clone().flat_map(|m| m.corners.iter().copied());
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        let (mut min, mut max) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for p in all {
            (lo, hi) = (lo.min(normal.dot(p)), hi.max(normal.dot(p)));
            (min, max) = (min.min(flat(p)), max.max(flat(p)));
        }
        let all = members.clone().flat_map(|m| m.corners.iter());
        let tol = tolerance(all);
        if hi - lo > tol {
            return None;
        }
        let polys: Vec<Vec<Vec2>> = (members.clone())
            .map(|m| m.corners.iter().map(|&p| flat(p)).collect())
            .collect();
        let area: f32 = polys.iter().map(|p| area(p).abs()).sum();
        if area * 4.0 < (max - min).x * (max - min).y {
            return None;
        }
        let (origin, texel, dims) = grid(min, max);
        let mut set = vec![false; (dims.x * dims.y) as usize];
        for poly in &polys {
            fill(&mut set, poly, (origin, texel, dims), tol);
        }
        let corners: Vec<&[Vec3]> = members.clone().map(|m| m.corners).collect();
        cover::union_fill(&mut set, (&polys, &corners), (origin, texel, dims), tol);
        let mut members: Vec<u32> = members.map(|m| m.mesh).collect();
        members.sort_unstable();
        members.dedup();
        Some(Self {
            normal,
            lo,
            hi,
            face: first.face,
            axes,
            origin,
            texel,
            dims,
            sums: sums(&set, dims),
            members,
        })
    }

    fn sparse(&self) -> bool {
        let total = self.dims.x as usize * self.dims.y as usize;
        let set = *self.sums.last().unwrap_or(&0) as usize;
        set * 4 < total
    }

    #[cfg(feature = "dev")]
    pub fn bytes(&self) -> usize {
        self.sums.len() * 4
    }

    pub fn holds(&self, object: u32) -> bool {
        self.members.binary_search(&object).is_ok()
    }

    pub fn hides(&self, from: (Vec3, Vec3), to: (Vec3, Vec3)) -> bool {
        let n = self.normal;
        let dir = (to.0 + to.1) * 0.5 - (from.0 + from.1) * 0.5;
        if !self.face.stops(n, dir) || !opposite(n, (self.lo, self.hi), from, to) {
            return false;
        }
        let ((near, k), (far, j)) = (corners(from), corners(to));
        let (mut min, mut max) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for &a in &near[..k] {
            for &b in &far[..j] {
                let (da, db) = (n.dot(a), n.dot(b));
                for plane in [self.lo, self.hi] {
                    let x = a + (b - a) * ((plane - da) / (db - da));
                    let at = Vec2::new(self.axes.0.dot(x), self.axes.1.dot(x));
                    (min, max) = (min.min(at), max.max(at));
                }
            }
        }
        self.covered(min - Vec2::splat(CLEAR), max + Vec2::splat(CLEAR))
    }

    fn covered(&self, min: Vec2, max: Vec2) -> bool {
        let lo = ((min - self.origin) / self.texel).floor();
        let hi = ((max - self.origin) / self.texel).floor();
        let inside = lo.cmpge(Vec2::ZERO).all() && hi.cmplt(self.dims.as_vec2()).all();
        if !inside || !lo.is_finite() || !hi.is_finite() {
            return false;
        }
        let (lo, hi) = (lo.as_uvec2(), hi.as_uvec2() + UVec2::ONE);
        let w = self.dims.x as usize + 1;
        let at = |p: UVec2| self.sums[p.y as usize * w + p.x as usize] as i64;
        let count = at(hi) - at(UVec2::new(lo.x, hi.y)) - at(UVec2::new(hi.x, lo.y)) + at(lo);
        count == i64::from((hi.x - lo.x) * (hi.y - lo.y))
    }
}

fn grid(min: Vec2, max: Vec2) -> (Vec2, f32, UVec2) {
    let mut texel = TEXEL;
    loop {
        let origin = (min / texel).floor() * texel;
        let dims = ((max - origin) / texel).ceil().max(Vec2::ONE).as_uvec2();
        if (dims.x as usize).saturating_mul(dims.y as usize) <= MAX_TEXELS {
            return (origin, texel, dims);
        }
        texel *= 2.0;
    }
}

fn axes(normal: Vec3) -> (Vec3, Vec3) {
    let a = [Vec3::X, Vec3::Y, Vec3::Z];
    let pick = a
        .into_iter()
        .min_by(|p, q| normal.dot(*p).abs().total_cmp(&normal.dot(*q).abs()));
    let a = pick.unwrap_or(Vec3::X);
    let u = (a - normal * normal.dot(a)).normalize_or_zero();
    (u, normal.cross(u))
}

fn fill(set: &mut [bool], poly: &[Vec2], (origin, texel, dims): (Vec2, f32, UVec2), tol: f32) {
    let k = poly.len();
    if k < 3 {
        return;
    }
    let turn = area(poly).signum();
    let inside = |p: Vec2| {
        (0..k).all(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % k]);
            turn * (b - a).perp_dot(p - a) >= -tol * (b - a).length()
        })
    };
    let (min, max) = poly.iter().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), &p| {
        (lo.min(p), hi.max(p))
    });
    let first = ((min - origin) / texel).floor().max(Vec2::ZERO).as_uvec2();
    let last = ((max - origin) / texel).ceil().as_uvec2().min(dims);
    for y in first.y..last.y {
        for x in first.x..last.x {
            let lo = origin + UVec2::new(x, y).as_vec2() * texel;
            let square = [
                lo,
                lo + Vec2::X * texel,
                lo + Vec2::Y * texel,
                lo + Vec2::splat(texel),
            ];
            if square.into_iter().all(inside) {
                set[(y * dims.x + x) as usize] = true;
            }
        }
    }
}

fn area(poly: &[Vec2]) -> f32 {
    let k = poly.len();
    (0..k)
        .map(|i| poly[i].perp_dot(poly[(i + 1) % k]))
        .sum::<f32>()
        * 0.5
}

fn sums(set: &[bool], dims: UVec2) -> Vec<u32> {
    let w = dims.x as usize + 1;
    let mut s = vec![0u32; w * (dims.y as usize + 1)];
    for y in 0..dims.y as usize {
        for x in 0..dims.x as usize {
            let v = u32::from(set[y * dims.x as usize + x]);
            s[(y + 1) * w + x + 1] = v + s[y * w + x + 1] + s[(y + 1) * w + x] - s[y * w + x];
        }
    }
    s
}
