//! From lightmap texels to surface points (#438): each receiving mesh's triangles are
//! rasterized in lightmap-UV space, and every texel a triangle covers becomes a
//! world-space point and normal to trace from. A texel whose centre a triangle
//! covers is traced at that centre. A texel a triangle only overlaps (#1687) is
//! traced at a point inside both the triangle and the texel ([`clip`]): at least
//! [`INSET`] off every edge where the overlap is that deep, else, on a triangle
//! thinner than that (a sliver, a tip), a quarter of its local width off every
//! edge, so a sliver owns every texel it crosses along its length. What is left to
//! dilation: an overlap shallower than the inset at the border of a wider triangle,
//! and texels no triangle touches. Dilation fills the ones bordering a chart so
//! filtering never reads black at a seam, and [`fill_rest`] the rest of the map
//! from its nearest chart, so no mip level averages the empty background in
//! (#1694). Before either, a texel the bake judged inside geometry is filled from
//! its own chart's valid texels ([`charts`], #1949).
//!
//! The walk reads a [`Surface`]: positions, normals, one UV set and the triangles.
//! The lightmap bake hands it a mesh's lightmap UV; the mesh-map baker
//! (`lighting::mesh_maps`, #1849) a model's UV0, so both rasterize and inset
//! the same way.

use std::ops::{Add, Div, Mul};

use glam::{Vec2, Vec3};

use super::input::BakeMesh;

/// A value a lightmap map holds texel by texel: a colour (`Vec3`), or a direction
/// texel (`Vec4`, the vector and its lamp share, #1695). Dilation, the fill, the
/// atlas and the mips only ever copy, add and scale one.
pub(super) trait Texel:
    Copy + Default + Add<Output = Self> + Mul<f32, Output = Self> + Div<f32, Output = Self>
{
}

impl<T> Texel for T where
    T: Copy + Default + Add<Output = T> + Mul<f32, Output = T> + Div<f32, Output = T>
{
}

mod charts;
#[cfg(test)]
mod charts_tests;
mod clip;

pub(super) use charts::{chart_of_triangles, fill_within, texel_charts};
use clip::Clipper;
#[cfg(test)]
pub(super) use clip::INSET;

/// Smallest lightmap edge, in texels.
pub const MIN_RESOLUTION: u32 = 4;

/// A covered texel: its index in the row-major image, where it sits on the mesh,
/// and the offset of the first index of the triangle it was taken from. Both
/// offsets are `u32` (no image or mesh comes near 2³² texels or indices), so a
/// point stays 32 bytes, as before the mesh-map baker added `first` (#1849): a
/// fully covered 4096² map holds 512 MiB of points, not 640.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TexelPoint {
    pub index: u32,
    pub position: Vec3,
    pub normal: Vec3,
    pub first: u32,
}

impl TexelPoint {
    /// [`index`](Self::index) as a slice index.
    pub fn at(&self) -> usize {
        self.index as usize
    }

    /// [`first`](Self::first) as a slice index.
    pub fn tri(&self) -> usize {
        self.first as usize
    }
}

/// What [`rasterize_surface`] walks: per-vertex positions, unit normals and the UV
/// set to rasterize in, and the triangle list indexing them.
#[derive(Clone, Copy)]
pub(crate) struct Surface<'a> {
    pub positions: &'a [Vec3],
    pub normals: &'a [Vec3],
    pub uvs: &'a [Vec2],
    pub indices: &'a [u32],
}

impl BakeMesh {
    /// The mesh as the lightmap bake rasterizes it: in its lightmap UV.
    pub(super) fn surface(&self) -> Surface<'_> {
        Surface {
            positions: &self.positions,
            normals: &self.normals,
            uvs: &self.lightmap_uvs,
            indices: &self.indices,
        }
    }
}

/// The square lightmap edge for `mesh`: `texels_per_unit` texels per world unit,
/// judged from how much world surface each unit of lightmap UV covers, clamped to
/// `[MIN_RESOLUTION, max]`. `None` when the lightmap UVs enclose no area (no second
/// UV map): that mesh keeps probe / ambient lighting.
pub fn lightmap_size(mesh: &BakeMesh, texels_per_unit: f32, max: u32) -> Option<u32> {
    let edge = world_per_uv(&mesh.surface())? * texels_per_unit;
    Some((edge.ceil() as u32).clamp(MIN_RESOLUTION, max.max(MIN_RESOLUTION)))
}

/// World units spanned by one unit of `mesh`'s UV (the square root of world area
/// over UV area); `None` when its UVs enclose no area.
pub(crate) fn world_per_uv(mesh: &Surface) -> Option<f32> {
    let (mut world, mut uv) = (0.0f32, 0.0f32);
    for t in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| i as usize);
        let (pa, pb, pc) = (mesh.positions[a], mesh.positions[b], mesh.positions[c]);
        world += (pb - pa).cross(pc - pa).length() * 0.5;
        let (ua, ub, uc) = (mesh.uvs[a], mesh.uvs[b], mesh.uvs[c]);
        uv += (ub - ua).perp_dot(uc - ua).abs() * 0.5;
    }
    (uv >= 1e-8 && world > 0.0).then(|| (world / uv).sqrt())
}

/// One triangle of a mesh in texel space: its first index's offset, its vertex
/// indices, its UVs scaled to texels, and their signed doubled area.
struct UvTri {
    first: usize,
    idx: [usize; 3],
    uv: [Vec2; 3],
    area: f32,
}

/// Every texel of a `size`² lightmap a triangle of `mesh` covers, in two passes.
/// First every texel whose centre a triangle covers, traced at that centre. Then,
/// conservatively (#1687), every texel still empty that a triangle overlaps with
/// area, traced where [`Clipper::trace_weights`] says: so a triangle
/// narrower than a texel, and a chart made only of such triangles, still owns
/// texels, each traced from a point strictly inside its own surface, never on an
/// edge where another surface may meet it. In either pass a texel two triangles
/// share goes to the first in index order, and the first pass never yields to the
/// second, so a centre-covered texel bakes exactly as it would without the second.
pub(super) fn rasterize(mesh: &BakeMesh, size: u32) -> Vec<TexelPoint> {
    rasterize_surface(&mesh.surface(), size)
}

/// [`rasterize`] over any [`Surface`]: its own UV set, scaled to a `size`² image.
pub(crate) fn rasterize_surface(mesh: &Surface, size: u32) -> Vec<TexelPoint> {
    let tris = uv_triangles(mesh, size as f32);
    let mut taken = vec![false; (size * size) as usize];
    let mut out = Vec::new();
    for conservative in [false, true] {
        for t in &tris {
            let clipper = conservative.then(|| Clipper::new(t));
            for (x, y) in texels_under(t, size) {
                let index = (y * size + x) as usize;
                if taken[index] {
                    continue;
                }
                let w = match &clipper {
                    None => barycentric(t.uv, t.area, Vec2::new(x as f32 + 0.5, y as f32 + 0.5)),
                    Some(c) => c.trace_weights(Vec2::new(x as f32, y as f32)),
                };
                let Some(w) = w else { continue };
                taken[index] = true;
                out.push(point(mesh, t, index as u32, w));
            }
        }
    }
    out
}

/// `mesh`'s triangles in texel space, those without UV area (or with a non-finite
/// UV) left out.
fn uv_triangles(mesh: &Surface, scale: f32) -> Vec<UvTri> {
    mesh.indices
        .chunks_exact(3)
        .enumerate()
        .filter_map(|(k, t)| {
            let idx = [t[0], t[1], t[2]].map(|i| i as usize);
            let uv = idx.map(|i| mesh.uvs[i] * scale);
            let area = (uv[1] - uv[0]).perp_dot(uv[2] - uv[0]);
            let first = 3 * k;
            (area.is_finite() && area.abs() >= 1e-12).then_some(UvTri {
                first,
                idx,
                uv,
                area,
            })
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
fn point(mesh: &Surface, t: &UvTri, index: u32, w: Vec3) -> TexelPoint {
    let [a, b, c] = t.idx;
    let position = w.x * mesh.positions[a] + w.y * mesh.positions[b] + w.z * mesh.positions[c];
    let normal = (w.x * mesh.normals[a] + w.y * mesh.normals[b] + w.z * mesh.normals[c])
        .try_normalize()
        .unwrap_or(Vec3::Y);
    TexelPoint {
        index,
        position,
        normal,
        first: t.first as u32,
    }
}

/// The barycentric weights of `p` in triangle `uv` (signed doubled area `area`), or
/// `None` when it lies outside.
fn barycentric(uv: [Vec2; 3], area: f32, p: Vec2) -> Option<Vec3> {
    let w0 = (uv[1] - p).perp_dot(uv[2] - p) / area;
    let w1 = (uv[2] - p).perp_dot(uv[0] - p) / area;
    let w = Vec3::new(w0, w1, 1.0 - w0 - w1);
    const EPS: f32 = -1e-5;
    (w.x >= EPS && w.y >= EPS && w.z >= EPS).then_some(w)
}

/// Fill each empty texel bordering filled ones with their average, `passes` rings
/// deep, so bilinear filtering at a chart's edge reads lit texels, not black.
pub(super) fn dilate<T: Texel>(texels: &mut [T], filled: &mut [bool], size: u32, passes: u32) {
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
                let (mut sum, mut n) = (T::default(), 0.0);
                for (dx, dy) in NEIGHBOURS {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= size || ny >= size {
                        continue;
                    }
                    let j = (ny * size + nx) as usize;
                    if snapshot[j] {
                        sum = sum + values[j];
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

/// Fill every texel still empty with the value of its nearest filled texel (in
/// four-neighbour steps, ties to the texel found first in row-major order), so a
/// mip level that averages a chart's border with the map's empty background reads
/// the chart's light, not black (#1694). A map with no filled texel stays as it is.
pub(super) fn fill_rest<T: Texel>(texels: &mut [T], filled: &[bool], size: u32) {
    let size = size as usize;
    let mut done = filled.to_vec();
    let mut queue: std::collections::VecDeque<usize> =
        (0..done.len()).filter(|&i| done[i]).collect();
    while let Some(i) = queue.pop_front() {
        let (x, y) = (i % size, i / size);
        let near = [
            (x > 0).then(|| i - 1),
            (x + 1 < size).then(|| i + 1),
            (y > 0).then(|| i - size),
            (y + 1 < size).then(|| i + size),
        ];
        for j in near.into_iter().flatten() {
            if !done[j] {
                texels[j] = texels[i];
                done[j] = true;
                queue.push_back(j);
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
