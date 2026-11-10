//! The texels a sheet's faces hold together though none holds alone (#1981): a
//! wall an exporter tessellated into triangles leaves every texel its diagonals
//! cross outside each one, so it fused nothing and the bake cast every ray
//! past it (a Kolvik-sized level baked 60 times slower than a box-built one).
//!
//! The members' union is bounded by the edges no other member shares from the
//! other side: one used as often in each direction (faces wound one way, so a
//! shared edge runs backward in its neighbour) has faces on both sides, and
//! lies inside. A texel that no boundary edge reaches (its square shrunk by the
//! noise `tol`) and whose centre a member holds lies wholly in the union, since
//! a path out of it would cross the boundary. Edges match only on corners that
//! are bit for bit the same, as a mesh's shared vertices are: a T-junction or a
//! seam welded to noise stays boundary, which fuses less and is never wrong.

use std::collections::BTreeMap;

use glam::{UVec2, Vec2, Vec3};

/// Set, in the grid `(origin, texel, dims)`, every texel the union of the convex
/// `polys` (on the plane, `corners` the same points in 3D) holds whole, its
/// edges missing a texel by up to `tol`.
pub(super) fn union_fill(
    set: &mut [bool],
    (polys, corners): (&[Vec<Vec2>], &[&[Vec3]]),
    (origin, texel, dims): (Vec2, f32, UVec2),
    tol: f32,
) {
    let bits = |p: Vec3| p.to_array().map(f32::to_bits);
    // Per undirected edge (its corners' bits, ordered), its 2D ends and how many
    // members run it one way less the other.
    let mut edges: BTreeMap<([u32; 3], [u32; 3]), (Vec2, Vec2, i32)> = BTreeMap::new();
    // A face with no area holds nothing, so it may not cancel an edge either.
    let solid = |poly: &&Vec<Vec2>| {
        let long = (0..poly.len()).map(|i| poly[i].distance(poly[(i + 1) % poly.len()]));
        super::area(poly).abs() > tol * long.fold(0.0, f32::max)
    };
    for (poly, at) in polys.iter().zip(corners).filter(|(p, _)| solid(p)) {
        let k = poly.len();
        let turn = if super::area(poly) < 0.0 { -1 } else { 1 };
        for i in 0..k {
            let (a, b) = (bits(at[i]), bits(at[(i + 1) % k]));
            let (key, way) = if a < b {
                ((a, b), turn)
            } else {
                ((b, a), -turn)
            };
            let e = edges.entry(key).or_insert((poly[i], poly[(i + 1) % k], 0));
            e.2 += way;
        }
    }
    let cell = |x: u32, y: u32| {
        let lo = origin + UVec2::new(x, y).as_vec2() * texel;
        (lo, lo + Vec2::splat(texel))
    };
    let mut cut = vec![false; set.len()];
    for &(a, b, _) in edges.values().filter(|e| e.2 != 0) {
        let (first, last) = span(a.min(b), a.max(b), (origin, texel, dims));
        for y in first.y..last.y {
            for x in first.x..last.x {
                let (lo, hi) = cell(x, y);
                if crosses(a, b, lo + Vec2::splat(tol), hi - Vec2::splat(tol)) {
                    cut[(y * dims.x + x) as usize] = true;
                }
            }
        }
    }
    for poly in polys.iter().filter(solid) {
        let turn = super::area(poly).signum();
        let k = poly.len();
        let holds = |p: Vec2| {
            (0..k).all(|i| {
                let (a, b) = (poly[i], poly[(i + 1) % k]);
                turn * (b - a).perp_dot(p - a) >= -tol * (b - a).length()
            })
        };
        let (min, max) = poly.iter().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), &p| {
            (lo.min(p), hi.max(p))
        });
        let (first, last) = span(min, max, (origin, texel, dims));
        for y in first.y..last.y {
            for x in first.x..last.x {
                let at = (y * dims.x + x) as usize;
                let (lo, hi) = cell(x, y);
                if !cut[at] && holds((lo + hi) * 0.5) {
                    set[at] = true;
                }
            }
        }
    }
}

/// The texels `[first, last)` the rectangle `(min, max)` reaches, a texel more
/// each way.
fn span(min: Vec2, max: Vec2, (origin, texel, dims): (Vec2, f32, UVec2)) -> (UVec2, UVec2) {
    let first = ((min - origin) / texel).floor() - Vec2::ONE;
    let last = ((max - origin) / texel).ceil() + Vec2::ONE;
    let first = first.max(Vec2::ZERO).as_uvec2().min(dims);
    (first, last.max(Vec2::ZERO).as_uvec2().min(dims))
}

/// Whether the segment `a`–`b` meets the box `(lo, hi)` (empty when inverted).
fn crosses(a: Vec2, b: Vec2, lo: Vec2, hi: Vec2) -> bool {
    if lo.cmpgt(hi).any() {
        return true;
    }
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for axis in 0..2 {
        if d[axis] == 0.0 {
            if a[axis] < lo[axis] || a[axis] > hi[axis] {
                return false;
            }
            continue;
        }
        let (mut near, mut far) = (
            (lo[axis] - a[axis]) / d[axis],
            (hi[axis] - a[axis]) / d[axis],
        );
        if near > far {
            (near, far) = (far, near);
        }
        (t0, t1) = (t0.max(near), t1.min(far));
        if t0 > t1 {
            return false;
        }
    }
    true
}
