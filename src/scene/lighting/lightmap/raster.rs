//! From lightmap texels to surface points (#438): each receiving mesh's triangles are
//! rasterized in lightmap-UV space, and every texel whose centre a triangle covers
//! becomes a world-space point and normal to trace from. Texels no triangle covers
//! stay empty; dilation fills the ones bordering a chart so filtering never reads
//! black at a seam.

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
    if uv < 1e-8 || world <= 0.0 {
        return None;
    }
    let edge = (world / uv).sqrt() * texels_per_unit;
    Some((edge.ceil() as u32).clamp(MIN_RESOLUTION, max.max(MIN_RESOLUTION)))
}

/// Every texel of a `size`² lightmap a triangle of `mesh` covers. A texel two
/// triangles share goes to the first.
pub(super) fn rasterize(mesh: &BakeMesh, size: u32) -> Vec<TexelPoint> {
    let mut taken = vec![false; (size * size) as usize];
    let mut out = Vec::new();
    let scale = size as f32;
    for t in mesh.indices.chunks_exact(3) {
        let idx = [t[0], t[1], t[2]].map(|i| i as usize);
        let uv = idx.map(|i| mesh.lightmap_uvs[i] * scale);
        let area = (uv[1] - uv[0]).perp_dot(uv[2] - uv[0]);
        if area.abs() < 1e-12 {
            continue;
        }
        let lo = uv[0].min(uv[1]).min(uv[2]).floor().max(Vec2::ZERO);
        let hi = uv[0].max(uv[1]).max(uv[2]).ceil().min(Vec2::splat(scale));
        for y in lo.y as u32..hi.y as u32 {
            for x in lo.x as u32..hi.x as u32 {
                let index = (y * size + x) as usize;
                if taken[index] {
                    continue;
                }
                let centre = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let Some(w) = barycentric(uv, area, centre) else {
                    continue;
                };
                taken[index] = true;
                let position = w.x * mesh.positions[idx[0]]
                    + w.y * mesh.positions[idx[1]]
                    + w.z * mesh.positions[idx[2]];
                let normal = (w.x * mesh.normals[idx[0]]
                    + w.y * mesh.normals[idx[1]]
                    + w.z * mesh.normals[idx[2]])
                    .try_normalize()
                    .unwrap_or(Vec3::Y);
                out.push(TexelPoint {
                    index,
                    position,
                    normal,
                });
            }
        }
    }
    out
}

/// The barycentric weights of `p` in triangle `uv` (signed doubled area `area`), or
/// `None` when it lies outside.
fn barycentric(uv: [Vec2; 3], area: f32, p: Vec2) -> Option<Vec3> {
    let w0 = (uv[1] - p).perp_dot(uv[2] - p) / area;
    let w1 = (uv[2] - p).perp_dot(uv[0] - p) / area;
    let w2 = 1.0 - w0 - w1;
    const EPS: f32 = -1e-5;
    (w0 >= EPS && w1 >= EPS && w2 >= EPS).then_some(Vec3::new(w0, w1, w2))
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
