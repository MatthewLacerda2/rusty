//! From lightmap texels to surface points (#438): each receiving mesh's triangles are
//! rasterized in lightmap-UV space, and every texel a triangle covers becomes a
//! world-space point and normal to trace from: at the texel's centre when the
//! triangle covers it, else at the triangle's point nearest it, so even a chart
//! thinner than a texel owns texels (#1687). Texels no triangle touches stay empty;
//! dilation fills the ones bordering a chart so filtering never reads black at a
//! seam.

use glam::{Vec2, Vec3};

use super::input::BakeMesh;

/// Smallest lightmap edge, in texels.
pub const MIN_RESOLUTION: u32 = 4;

/// A covered texel: its index in the row-major image, and where it sits on the mesh.
#[derive(Clone, Copy, Debug)]
pub(super) struct TexelPoint {
    pub index: usize,
    pub position: Vec3,
    pub normal: Vec3,
}

/// The square lightmap edge for `mesh`: `texels_per_unit` texels per world unit,
/// judged from how much world surface each unit of lightmap UV covers, clamped to
/// `[MIN_RESOLUTION, max]`. `None` when the lightmap UVs enclose no area (no second
/// UV map): that mesh keeps probe / ambient lighting.
pub fn lightmap_size(mesh: &BakeMesh, texels_per_unit: f32, max: u32) -> Option<u32> {
    let edge = world_per_uv(mesh)? * texels_per_unit;
    Some((edge.ceil() as u32).clamp(MIN_RESOLUTION, max.max(MIN_RESOLUTION)))
}

/// World units spanned by one unit of `mesh`'s lightmap UV (the square root of world
/// area over UV area); `None` when the lightmap UVs enclose no area.
pub(super) fn world_per_uv(mesh: &BakeMesh) -> Option<f32> {
    let (mut world, mut uv) = (0.0f32, 0.0f32);
    for t in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| i as usize);
        let (pa, pb, pc) = (mesh.positions[a], mesh.positions[b], mesh.positions[c]);
        world += (pb - pa).cross(pc - pa).length() * 0.5;
        let (ua, ub, uc) = (
            mesh.lightmap_uvs[a],
            mesh.lightmap_uvs[b],
            mesh.lightmap_uvs[c],
        );
        uv += (ub - ua).perp_dot(uc - ua).abs() * 0.5;
    }
    (uv >= 1e-8 && world > 0.0).then(|| (world / uv).sqrt())
}

/// One triangle of a mesh in lightmap texel space: its vertex indices, its UVs
/// scaled to texels, and their signed doubled area.
struct UvTri {
    idx: [usize; 3],
    uv: [Vec2; 3],
    area: f32,
}

/// Every texel of a `size`² lightmap a triangle of `mesh` covers, in two passes.
/// First every texel whose centre a triangle covers, traced at that centre. Then,
/// conservatively (#1687), every texel still empty that a triangle overlaps, traced
/// at the triangle's point nearest the texel centre: so a triangle narrower than a
/// texel, and a chart made only of such triangles, still owns texels, each traced
/// from its own surface. In either pass a texel two triangles share goes to the
/// first in index order, and the first pass never yields to the second, so a
/// centre-covered texel bakes exactly as it would without the second pass.
pub(super) fn rasterize(mesh: &BakeMesh, size: u32) -> Vec<TexelPoint> {
    let tris = uv_triangles(mesh, size as f32);
    let mut taken = vec![false; (size * size) as usize];
    let mut out = Vec::new();
    for conservative in [false, true] {
        for t in &tris {
            for (x, y) in texels_under(t, size) {
                let index = (y * size + x) as usize;
                if taken[index] {
                    continue;
                }
                let centre = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let w = match conservative {
                    false => barycentric(t.uv, t.area, centre),
                    true => overlaps(t.uv, centre).then(|| nearest_weights(t, centre)),
                };
                let Some(w) = w else { continue };
                taken[index] = true;
                out.push(point(mesh, t, index, w));
            }
        }
    }
    out
}

/// `mesh`'s triangles in texel space, those without UV area (or with a non-finite
/// UV) left out.
fn uv_triangles(mesh: &BakeMesh, scale: f32) -> Vec<UvTri> {
    mesh.indices
        .chunks_exact(3)
        .filter_map(|t| {
            let idx = [t[0], t[1], t[2]].map(|i| i as usize);
            let uv = idx.map(|i| mesh.lightmap_uvs[i] * scale);
            let area = (uv[1] - uv[0]).perp_dot(uv[2] - uv[0]);
            (area.is_finite() && area.abs() >= 1e-12).then_some(UvTri { idx, uv, area })
        })
        .collect()
}

/// The texels of a `size`² image that `t`'s bounding box overlaps, row by row.
fn texels_under(t: &UvTri, size: u32) -> impl Iterator<Item = (u32, u32)> {
    let scale = Vec2::splat(size as f32);
    let lo = t.uv[0].min(t.uv[1]).min(t.uv[2]).floor().max(Vec2::ZERO);
    let hi = t.uv[0].max(t.uv[1]).max(t.uv[2]).ceil().min(scale);
    let (x0, x1) = (lo.x as u32, hi.x as u32);
    (lo.y as u32..hi.y as u32).flat_map(move |y| (x0..x1).map(move |x| (x, y)))
}

/// The surface point of triangle `t` at barycentric weights `w`, for texel `index`.
fn point(mesh: &BakeMesh, t: &UvTri, index: usize, w: Vec3) -> TexelPoint {
    let [a, b, c] = t.idx;
    let position = w.x * mesh.positions[a] + w.y * mesh.positions[b] + w.z * mesh.positions[c];
    let normal = (w.x * mesh.normals[a] + w.y * mesh.normals[b] + w.z * mesh.normals[c])
        .try_normalize()
        .unwrap_or(Vec3::Y);
    TexelPoint {
        index,
        position,
        normal,
    }
}

/// The barycentric weights of `p` in triangle `uv` (signed doubled area `area`), or
/// `None` when it lies outside.
fn barycentric(uv: [Vec2; 3], area: f32, p: Vec2) -> Option<Vec3> {
    let w = weights(uv, area, p);
    const EPS: f32 = -1e-5;
    (w.x >= EPS && w.y >= EPS && w.z >= EPS).then_some(w)
}

/// The barycentric weights of `p` in triangle `uv`, inside or not.
fn weights(uv: [Vec2; 3], area: f32, p: Vec2) -> Vec3 {
    let w0 = (uv[1] - p).perp_dot(uv[2] - p) / area;
    let w1 = (uv[2] - p).perp_dot(uv[0] - p) / area;
    Vec3::new(w0, w1, 1.0 - w0 - w1)
}

/// Whether triangle `uv` overlaps the unit texel square centred on `centre` with
/// some area (touching an edge or corner is not overlapping): the separating-axis
/// test on the square's two axes and the triangle's three edge normals.
fn overlaps(uv: [Vec2; 3], centre: Vec2) -> bool {
    let axes = [
        Vec2::X,
        Vec2::Y,
        uv[1] - uv[0],
        uv[2] - uv[1],
        uv[0] - uv[2],
    ];
    axes.iter().enumerate().all(|(k, &axis)| {
        let n = if k < 2 { axis } else { axis.perp() };
        let (lo, hi) = uv
            .iter()
            .map(|p| n.dot(*p - centre))
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), d| {
                (lo.min(d), hi.max(d))
            });
        let reach = 0.5 * (n.x.abs() + n.y.abs());
        lo < reach && hi > -reach
    })
}

/// The barycentric weights of the point of triangle `t` nearest `p`, clamped onto
/// the triangle so its surface point is never extrapolated off it.
fn nearest_weights(t: &UvTri, p: Vec2) -> Vec3 {
    if let Some(w) = barycentric(t.uv, t.area, p) {
        return clamped(w);
    }
    let edges = [(0, 1), (1, 2), (2, 0)];
    let nearest = edges
        .map(|(i, j)| {
            let (a, b) = (t.uv[i], t.uv[j]);
            let ab = b - a;
            let s = ((p - a).dot(ab) / ab.length_squared().max(1e-20)).clamp(0.0, 1.0);
            a + ab * s
        })
        .into_iter()
        .min_by(|a, b| a.distance_squared(p).total_cmp(&b.distance_squared(p)))
        .unwrap_or(t.uv[0]);
    clamped(weights(t.uv, t.area, nearest))
}

/// `w` with float error off the triangle removed: no weight below zero, and they
/// sum to one.
fn clamped(w: Vec3) -> Vec3 {
    let w = w.max(Vec3::ZERO);
    let sum = w.x + w.y + w.z;
    if sum > 0.0 {
        w / sum
    } else {
        Vec3::splat(1.0 / 3.0)
    }
}

/// Fill each empty texel bordering filled ones with their average, `passes` rings
/// deep, so bilinear filtering at a chart's edge reads lit texels, not black.
pub(super) fn dilate(texels: &mut [Vec3], filled: &mut [bool], size: u32, passes: u32) {
    let size = size as i32;
    for _ in 0..passes {
        let snapshot = filled.to_vec();
        let values = texels.to_vec();
        for y in 0..size {
            for x in 0..size {
                let i = (y * size + x) as usize;
                if snapshot[i] {
                    continue;
                }
                let (mut sum, mut n) = (Vec3::ZERO, 0.0);
                for (dx, dy) in NEIGHBOURS {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= size || ny >= size {
                        continue;
                    }
                    let j = (ny * size + nx) as usize;
                    if snapshot[j] {
                        sum += values[j];
                        n += 1.0;
                    }
                }
                if n > 0.0 {
                    texels[i] = sum / n;
                    filled[i] = true;
                }
            }
        }
    }
}

const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];
