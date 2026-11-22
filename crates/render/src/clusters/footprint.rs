use glam::Vec3;

use super::{ClusterGrid, GRID};

/// A cluster range: the tiles across and up (`[first, last]` each) and the slices.
pub(super) type Footprint = ([[u32; 2]; 2], [u32; 2]);

pub(super) fn footprint(grid: &ClusterGrid, center: Vec3, radius: f32) -> Option<Footprint> {
    if radius.is_nan() || radius <= 0.0 {
        return None;
    }
    let depth = -center.z;
    if depth + radius < grid.near || depth - radius > grid.far {
        return None;
    }
    let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    for corner in 0..8 {
        let sign = |bit: u32| if corner & bit == 0 { -1.0 } else { 1.0 };
        let p = center + Vec3::new(sign(1), sign(2), sign(4)) * radius;
        let ndc = grid.ndc(p);
        (lo, hi) = (lo.min(ndc), hi.max(ndc));
    }
    if hi.x < -1.0 || hi.y < -1.0 || lo.x > 1.0 || lo.y > 1.0 {
        return None;
    }
    let tile =
        |ndc: f32, n: u32| (((ndc * 0.5 + 0.5) * n as f32).floor().max(0.0) as u32).min(n - 1);
    let slice = |d: f32| (grid.slice_of(d).floor().max(0.0) as u32).min(GRID[2] - 1);
    Some((
        [
            [tile(lo.x, GRID[0]), tile(hi.x, GRID[0])],
            [tile(lo.y, GRID[1]), tile(hi.y, GRID[1])],
        ],
        [slice(depth - radius), slice((depth + radius).min(grid.far))],
    ))
}
