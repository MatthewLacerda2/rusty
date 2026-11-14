//! The texels a sheet's faces hold together though none holds alone (#1981): a
//! wall an exporter tessellated into triangles leaves every texel its diagonals
//! cross outside each one, so it fused nothing and the bake cast every ray
//! past it. A primitive town of such walls baked 7 to 8 times slower than its
//! box-built twin; Kolvik baked 60 times slower than cozy-coffee, a cause this
//! fits but no one has confirmed on Kolvik itself.
//!
//! The members' union is bounded by the edges no other member shares from the
//! other side: one used as often in each direction (faces wound one way, so a
//! shared edge runs backward in its neighbour) has faces on both sides, and
//! lies inside. A texel that no boundary edge reaches (its square shrunk by the
//! noise `tol`) and whose centre a member holds, with no slack, lies wholly in
//! the union, since a path out of it would cross the boundary. (Slack there
//! would reach past a thin face's tip, far beyond `tol`, into an opening.) A
//! centre on an edge two members share is held by one side or the other. Edges match only on corners that
//! are bit for bit the same, as a mesh's shared vertices are: a T-junction or a
//! seam welded to noise stays boundary, which fuses less and is never wrong.

#[cfg(all(test, feature = "dev"))]
use std::cell::Cell;
use std::collections::BTreeMap;

use glam::{UVec2, Vec2, Vec3};

/// A sheet's texel grid: origin, texel edge, size.
type TexelGrid = (Vec2, f32, UVec2);
/// An edge's corners' bits, ordered: the same edge whichever way it runs.
type EdgeKey = ([u32; 3], [u32; 3]);

/// Set, in `grid`, every texel the union of the convex `polys` (on the plane,
/// `corners` the same points in 3D) holds whole, its edges missing a texel by up
/// to `tol`.
pub(super) fn union_fill(
    set: &mut [bool],
    (polys, corners): (&[Vec<Vec2>], &[&[Vec3]]),
    grid: TexelGrid,
    tol: f32,
) {
    // A face with no area holds nothing, so it may not cancel an edge either.
    let solid = |poly: &Vec<Vec2>| {
        let long = (0..poly.len()).map(|i| poly[i].distance(poly[(i + 1) % poly.len()]));
        super::area(poly).abs() > tol * long.fold(0.0, f32::max)
    };
    let faces = polys.iter().zip(corners).filter(|(p, _)| solid(p));
    // With no edge cancelled the union is the members apart, which `fill` set.
    let Some(boundary) = boundary(faces).filter(|_| on()) else {
        return;
    };
    let cut = cut(set.len(), boundary, grid, tol);
    for poly in polys.iter().filter(|p| solid(p)) {
        let turn = super::area(poly).signum();
        let k = poly.len();
        let holds = |p: Vec2| {
            (0..k).all(|i| {
                let (a, b) = (poly[i], poly[(i + 1) % k]);
                turn * (b - a).perp_dot(p - a) >= 0.0
            })
        };
        let (min, max) = poly.iter().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), &p| {
            (lo.min(p), hi.max(p))
        });
        let (first, last) = span(min, max, grid);
        for y in first.y..last.y {
            for x in first.x..last.x {
                let at = (y * grid.2.x + x) as usize;
                let (lo, hi) = cell(grid, x, y);
                if !cut[at] && holds((lo + hi) * 0.5) {
                    set[at] = true;
                }
            }
        }
    }
}

#[cfg(all(test, feature = "dev"))]
thread_local! {
    static OFF: Cell<bool> = const { Cell::new(false) };
}

/// Whether faces fuse by their union: always, outside [`without_union`].
fn on() -> bool {
    #[cfg(all(test, feature = "dev"))]
    if OFF.with(Cell::get) {
        return false;
    }
    true
}

/// `make()` with sheets made as before #1981, each face filling alone, on this
/// thread: a test's baseline for what the union may never change.
#[cfg(all(test, feature = "dev"))]
pub(in crate::occlusion) fn without_union<T>(make: impl FnOnce() -> T) -> T {
    OFF.with(|off| off.set(true));
    let out = make();
    OFF.with(|off| off.set(false));
    out
}

/// The union's boundary: each edge (its 2D ends) that `faces` do not run as
/// often one way as the other; `None` when every edge is, as no edge cancels.
fn boundary<'a>(
    faces: impl Iterator<Item = (&'a Vec<Vec2>, &'a &'a [Vec3])>,
) -> Option<Vec<(Vec2, Vec2)>> {
    let bits = |p: Vec3| p.to_array().map(f32::to_bits);
    let mut edges: BTreeMap<EdgeKey, (Vec2, Vec2, i32)> = BTreeMap::new();
    for (poly, at) in faces {
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
    let all = edges.len();
    let open: Vec<_> = (edges.into_values())
        .filter(|e| e.2 != 0)
        .map(|e| (e.0, e.1))
        .collect();
    (open.len() < all).then_some(open)
}

/// Per texel of `grid` (`count` of them), whether a `boundary` edge reaches its
/// square shrunk by `tol`.
fn cut(count: usize, boundary: Vec<(Vec2, Vec2)>, grid: TexelGrid, tol: f32) -> Vec<bool> {
    let mut cut = vec![false; count];
    for (a, b) in boundary {
        let (first, last) = span(a.min(b), a.max(b), grid);
        for y in first.y..last.y {
            for x in first.x..last.x {
                let (lo, hi) = cell(grid, x, y);
                if crosses(a, b, lo + Vec2::splat(tol), hi - Vec2::splat(tol)) {
                    cut[(y * grid.2.x + x) as usize] = true;
                }
            }
        }
    }
    cut
}

/// Texel `(x, y)`'s square.
fn cell((origin, texel, _): TexelGrid, x: u32, y: u32) -> (Vec2, Vec2) {
    let lo = origin + UVec2::new(x, y).as_vec2() * texel;
    (lo, lo + Vec2::splat(texel))
}

/// The texels `[first, last)` the rectangle `(min, max)` reaches, a texel more
/// each way.
fn span(min: Vec2, max: Vec2, (origin, texel, dims): TexelGrid) -> (UVec2, UVec2) {
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
