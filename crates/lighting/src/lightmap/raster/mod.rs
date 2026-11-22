//! A texel whose centre a triangle covers is traced there; one it only overlaps, inside both
//! triangle and texel ([`clip`]). Dilation and [`fill_rest`] fill the rest.

use glam::{Vec2, Vec3};

use super::input::BakeMesh;

pub(super) use rusty_scene::lighting::page::Texel;

mod areas;
mod charts;
#[cfg(test)]
mod charts_tests;
mod clip;
mod fill;
mod overlap;

pub(crate) use areas::{world_per_uv, Areas};
pub(super) use charts::{chart_of_triangles, fill_within, texel_charts};
use clip::Clipper;
#[cfg(test)]
pub(super) use clip::INSET;
pub(super) use fill::{dilate, fill_rest};
pub(crate) use overlap::{rasterize_claims, Claim};
use overlap::{Owner, Owners};

pub const MIN_RESOLUTION: u32 = 4;

/// Offsets are `u32` so a point stays 32 bytes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TexelPoint {
    pub index: u32,
    pub position: Vec3,
    pub normal: Vec3,
    pub first: u32,
}

impl TexelPoint {
    pub fn at(&self) -> usize {
        self.index as usize
    }

    pub fn tri(&self) -> usize {
        self.first as usize
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Surface<'a> {
    pub positions: &'a [Vec3],
    pub normals: &'a [Vec3],
    pub uvs: &'a [Vec2],
    pub indices: &'a [u32],
}

impl BakeMesh {
    pub(super) fn surface(&self) -> Surface<'_> {
        Surface {
            positions: &self.positions,
            normals: &self.normals,
            uvs: &self.lightmap_uvs,
            indices: &self.indices,
        }
    }
}

/// `None` when the UVs enclose no area (no second UV map).
pub fn lightmap_size(mesh: &BakeMesh, texels_per_unit: f32, max: u32) -> Option<u32> {
    let edge = world_per_uv(&mesh.surface())? * texels_per_unit;
    Some((edge.ceil() as u32).clamp(MIN_RESOLUTION, max.max(MIN_RESOLUTION)))
}

struct UvTri {
    first: usize,
    idx: [usize; 3],
    uv: [Vec2; 3],
    area: f32,
}

/// A texel two triangles share goes to the first in index order; the second pass never takes a
/// texel from the first.
pub(super) fn rasterize(mesh: &BakeMesh, size: u32) -> Vec<TexelPoint> {
    rasterize_surface(&mesh.surface(), size, size)
}

pub(super) fn rasterize_counted(mesh: &BakeMesh, size: u32, count: usize) -> Vec<TexelPoint> {
    walk(
        &mesh.surface(),
        (size, size),
        Vec::with_capacity(count),
        None,
    )
}

pub(crate) fn rasterize_surface(mesh: &Surface, width: u32, height: u32) -> Vec<TexelPoint> {
    walk(mesh, (width, height), Vec::new(), None)
}

/// The owners are the same either way, and the sink may replace a texel's point, never its owner.
fn walk(
    mesh: &Surface,
    (width, height): (u32, u32),
    mut out: Vec<TexelPoint>,
    mut claims: Option<&mut dyn FnMut(Claim)>,
) -> Vec<TexelPoint> {
    let scale = Vec2::new(width as f32, height as f32);
    let tris = uv_triangles(mesh, scale);
    let texels = width as usize * height as usize;
    let mut owners = Owners::new(texels, claims.is_some());
    let mut asked = overlap::Asked::default();
    for conservative in [false, true] {
        let pass = out.len();
        let clip = |t| conservative.then(|| Clipper::new(t));
        for t in &tris {
            let clipper = clip(t);
            for (x, y) in texels_under(t, width, height) {
                let index = (y * width + x) as usize;
                let owner = owners.of(index);
                match owner {
                    Owner::Taken => continue,
                    Owner::At(slot, _) if (slot as usize) < pass => continue,
                    Owner::At(_, first) if !asked.overlaps(mesh, (t.first, t.uv), first, scale) => {
                        continue
                    }
                    Owner::At(..) => {}
                    Owner::Free => {}
                }
                let w = match &clipper {
                    None => barycentric(t.uv, t.area, Vec2::new(x as f32 + 0.5, y as f32 + 0.5)),
                    Some(c) => c.trace_weights(Vec2::new(x as f32, y as f32)),
                };
                let Some(w) = w else { continue };
                let p = point(mesh, t, index as u32, w);
                match (owner, claims.as_deref_mut()) {
                    (Owner::At(slot, first), Some(sink)) => sink(Claim {
                        slot,
                        owner: first,
                        point: p,
                        kept: &mut out[slot as usize],
                    }),
                    _ => {
                        owners.take(index, out.len(), p.first);
                        out.push(p);
                    }
                }
            }
        }
    }
    out
}

fn uv_triangles(mesh: &Surface, scale: Vec2) -> Vec<UvTri> {
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

fn texels_under(t: &UvTri, width: u32, height: u32) -> impl Iterator<Item = (u32, u32)> {
    let scale = Vec2::new(width as f32, height as f32);
    let lo = t.uv[0].min(t.uv[1]).min(t.uv[2]).floor().max(Vec2::ZERO);
    let hi = t.uv[0].max(t.uv[1]).max(t.uv[2]).ceil().min(scale);
    let (x0, x1) = (lo.x as u32, hi.x as u32);
    (lo.y as u32..hi.y as u32).flat_map(move |y| (x0..x1).map(move |x| (x, y)))
}

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

fn barycentric(uv: [Vec2; 3], area: f32, p: Vec2) -> Option<Vec3> {
    let w0 = (uv[1] - p).perp_dot(uv[2] - p) / area;
    let w1 = (uv[2] - p).perp_dot(uv[0] - p) / area;
    let w = Vec3::new(w0, w1, 1.0 - w0 - w1);
    const EPS: f32 = -1e-5;
    (w.x >= EPS && w.y >= EPS && w.z >= EPS).then_some(w)
}
