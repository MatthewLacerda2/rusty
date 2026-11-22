//! Gaussian filter on the indirect term (Unity's lightmap filter). A neighbour counts only if it
//! faces the same way and sits on the same stretch of surface, so the blur never crosses a crease
//! or a gap. Baked direct light is never filtered.

use glam::Vec3;

use super::raster::TexelPoint;

const MIN_NORMAL_COS: f32 = 0.9;

/// `radius == 0` returns `points` unchanged; `texel_world` is the texel's edge in world units.
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
        slot[p.at()] = k;
    }
    let r = radius as i32;
    let sigma = radius as f32 * 0.5 + 0.5;
    // Two texels' worth of slack over the kernel's reach, for stretched UVs.
    let reach = (radius as f32 + 2.0) * texel_world * 1.5;
    points
        .iter()
        .map(|p| {
            let (x, y) = ((p.index % size) as i32, (p.index / size) as i32);
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
