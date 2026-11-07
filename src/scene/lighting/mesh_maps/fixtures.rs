//! Primitive meshes the mesh-map tests bake (#1849): a unit cube and an open L,
//! each face a quad with flat normals and its own rectangle of UV0.

use glam::{Vec2, Vec3};

use super::MeshSource;

/// Append the quad `c` (counter-clockwise seen from where `normal` points) mapped to
/// the UV rectangle `lo..hi`, as its own material run when `new_run`.
pub(super) fn quad(m: &mut MeshSource, c: [Vec3; 4], uv: (Vec2, Vec2), new_run: bool) {
    let normal = (c[1] - c[0]).cross(c[2] - c[0]).normalize();
    let (lo, hi) = uv;
    let base = m.positions.len() as u32;
    let uvs = [lo, Vec2::new(hi.x, lo.y), hi, Vec2::new(lo.x, hi.y)];
    for (p, t) in c.into_iter().zip(uvs) {
        m.positions.push(p);
        m.normals.push(normal);
        m.uvs.push(t);
    }
    m.indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
    let end = m.indices.len() as u32;
    match (new_run, m.run_ends.last_mut()) {
        (false, Some(last)) => *last = end,
        _ => m.run_ends.push(end),
    }
}

/// The UV cell `k` of a 3×2 grid, inset by a gutter so islands never touch.
pub(super) fn cell(k: usize) -> (Vec2, Vec2) {
    let lo = Vec2::new((k % 3) as f32 / 3.0, (k / 3) as f32 / 2.0);
    let gutter = Vec2::splat(0.03);
    (lo + gutter, lo + Vec2::new(1.0 / 3.0, 0.5) - gutter)
}

/// The cube `[0, 1]³`, faces outward, each in its own [`cell`].
pub(super) fn cube() -> MeshSource {
    let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    let faces = [
        [v(0., 1., 0.), v(0., 1., 1.), v(1., 1., 1.), v(1., 1., 0.)], // +Y
        [v(0., 0., 0.), v(1., 0., 0.), v(1., 0., 1.), v(0., 0., 1.)], // -Y
        [v(1., 0., 0.), v(1., 1., 0.), v(1., 1., 1.), v(1., 0., 1.)], // +X
        [v(0., 0., 0.), v(0., 0., 1.), v(0., 1., 1.), v(0., 1., 0.)], // -X
        [v(0., 0., 1.), v(1., 0., 1.), v(1., 1., 1.), v(0., 1., 1.)], // +Z
        [v(0., 0., 0.), v(0., 1., 0.), v(1., 1., 0.), v(1., 0., 0.)], // -Z
    ];
    let mut m = MeshSource::default();
    for (k, f) in faces.into_iter().enumerate() {
        quad(&mut m, f, cell(k), false);
    }
    m
}

/// An open L: a floor `x, z ∈ [0, 1]` facing up in UV cell 0, and a wall at `x = 0`
/// facing +X in cell 1, meeting in a concave edge along z. The wall is its own
/// material slot.
pub(super) fn l_shape() -> MeshSource {
    let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    let mut m = MeshSource::default();
    let floor = [v(0., 0., 0.), v(0., 0., 1.), v(1., 0., 1.), v(1., 0., 0.)];
    let wall = [v(0., 0., 0.), v(0., 1., 0.), v(0., 1., 1.), v(0., 0., 1.)];
    quad(&mut m, floor, cell(0), false);
    quad(&mut m, wall, cell(1), true);
    m
}
