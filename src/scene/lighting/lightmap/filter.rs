//! Smoothing the baked bounce (#438), Unity's lightmap *Gaussian filter* on the
//! indirect term. A few hundred paths per texel still leave grain where the bounce
//! comes from a small bright patch, so the indirect light is blurred across
//! neighbouring texels, weighted so it never crosses a crease or a gap: a neighbour
//! counts only if it faces the same way and sits on the same stretch of surface.
//! Baked direct light is never filtered, so its shadows stay sharp.

use glam::Vec3;

use super::raster::TexelPoint;

/// A neighbour must face within this cosine of the texel's own normal.
const MIN_NORMAL_COS: f32 = 0.9;

/// The indirect light of `points` (one value each, in order), blurred over a
/// `radius`-texel Gaussian on a `size`² lightmap whose texels are `texel_world`
/// world units across. `radius == 0` returns it unchanged.
pub(super) fn smooth(
    points: &[TexelPoint],
    indirect: &[Vec3],
    size: u32,
    radius: u32,
    texel_world: f32,
) -> Vec<Vec3> {
    if radius == 0 {
        return indirect.to_vec();
    }
    let mut slot = vec![usize::MAX; (size * size) as usize];
    for (k, p) in points.iter().enumerate() {
        slot[p.index] = k;
    }
    let r = radius as i32;
    let sigma = radius as f32 * 0.5 + 0.5;
    // Two texels' worth of slack over the kernel's reach, for stretched UVs.
    let reach = (radius as f32 + 2.0) * texel_world * 1.5;
    points
        .iter()
        .map(|p| {
            let (x, y) = (
                (p.index as u32 % size) as i32,
                (p.index as u32 / size) as i32,
            );
            let (mut sum, mut weight) = (Vec3::ZERO, 0.0);
            for dy in -r..=r {
                for dx in -r..=r {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= size as i32 || ny >= size as i32 {
                        continue;
                    }
                    let k = slot[(ny as u32 * size + nx as u32) as usize];
                    if k == usize::MAX {
                        continue;
                    }
                    let q = &points[k];
                    let facing = p.normal.dot(q.normal);
                    if facing < MIN_NORMAL_COS || p.position.distance(q.position) > reach {
                        continue;
                    }
                    let w = (-((dx * dx + dy * dy) as f32) / (2.0 * sigma * sigma)).exp();
                    sum += indirect[k] * w;
                    weight += w;
                }
            }
            sum / weight
        })
        .collect()
}
