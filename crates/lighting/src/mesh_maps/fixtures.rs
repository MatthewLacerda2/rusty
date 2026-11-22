use glam::{Vec2, Vec3};

use super::{bake, MeshMap, MeshSource, Settings};
use rusty_procgen::image_buf::Image;

pub(super) fn bake_all(
    mesh: &MeshSource,
    maps: &[MeshMap],
    size: u32,
) -> Result<Vec<(MeshMap, Image)>, String> {
    bake_with(mesh, maps, &Settings::new(size))
}

pub(super) fn bake_with(
    mesh: &MeshSource,
    maps: &[MeshMap],
    settings: &Settings,
) -> Result<Vec<(MeshMap, Image)>, String> {
    let mut out = Vec::new();
    bake(mesh, maps, settings, |map, img| {
        out.push((map, img));
        Ok(())
    })?;
    Ok(out)
}

pub(super) fn quad(m: &mut MeshSource, c: [Vec3; 4], uv: (Vec2, Vec2), material: usize) {
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
    match m.runs.last_mut() {
        Some(last) if last.1 == Some(material) => last.0 = end,
        _ => m.runs.push((end, Some(material))),
    }
}

pub(super) fn cell(k: usize) -> (Vec2, Vec2) {
    let lo = Vec2::new((k % 3) as f32 / 3.0, (k / 3) as f32 / 2.0);
    let gutter = Vec2::splat(0.03);
    (lo + gutter, lo + Vec2::new(1.0 / 3.0, 0.5) - gutter)
}

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
        quad(&mut m, f, cell(k), 0);
    }
    m
}

pub(super) fn l_shape() -> MeshSource {
    let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    let mut m = MeshSource::default();
    let floor = [v(0., 0., 0.), v(0., 0., 1.), v(1., 0., 1.), v(1., 0., 0.)];
    let wall = [v(0., 0., 0.), v(0., 1., 0.), v(0., 1., 1.), v(0., 0., 1.)];
    quad(&mut m, floor, cell(0), 0);
    quad(&mut m, wall, cell(1), 1);
    m
}
