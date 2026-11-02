//! Texels claimed by overlap are traced strictly inside their triangle (#1687): a
//! point on an edge sits on whatever surface meets the mesh there, so rays from a
//! floor's border would leak through the wall it stops at.

use glam::{Vec2, Vec3};

use super::raster::rasterize;
use super::tests::quad;
use super::*;

/// The lightmap edge.
const S: u32 = 16;

#[test]
fn a_needle_is_traced_where_it_crosses_each_texel_not_far_along_it() {
    // UV area ~1e-4 texels²: weights recovered by dividing by it would wander.
    let uv = [(0.2, 0.5), (15.8, 0.5), (15.8, 0.50001)];
    let mesh = BakeMesh {
        positions: uv.map(|(u, v)| Vec3::new(u, 0.0, v)).to_vec(),
        normals: vec![Vec3::Y; 3],
        lightmap_uvs: uv.map(|(u, v)| Vec2::new(u, v) / S as f32).to_vec(),
        indices: vec![0, 1, 2],
        ..Default::default()
    };
    let points = rasterize(&mesh, S);
    assert!(!points.is_empty(), "the needle owns texels");
    for p in points {
        let x = (p.index % S as usize) as f32;
        let u = p.position.x;
        assert!((x..=x + 1.0).contains(&u), "texel {x} traced at u = {u}");
    }
}

#[test]
fn a_non_finite_uv_owns_nothing() {
    let mut mesh = BakeMesh {
        positions: vec![Vec3::ZERO, Vec3::X, Vec3::Z],
        normals: vec![Vec3::Y; 3],
        lightmap_uvs: vec![Vec2::ZERO, Vec2::new(f32::NAN, 0.0), Vec2::Y],
        indices: vec![0, 2, 1],
        ..Default::default()
    };
    assert!(rasterize(&mesh, S).is_empty());
    mesh.lightmap_uvs[1] = Vec2::new(f32::INFINITY, 0.0);
    assert!(rasterize(&mesh, S).is_empty());
}

/// Texel `u` of the floor's lightmap at world `x` (and `v` at `z`): its edges at
/// x = 0 and z = 0 cross the first column and row left of their centres, so those
/// texels are claimed by overlap, not by a centre.
fn texel(x: f32) -> f32 {
    0.3 + 3.6 * x
}

#[test]
fn a_floor_border_at_a_single_sided_wall_reads_its_shadow_not_the_sun_behind() {
    let p = |x, z| Vec3::new(x, 0.0, z);
    let mut floor = quad(
        1,
        [p(0.0, 4.0), p(4.0, 4.0), p(4.0, 0.0), p(0.0, 0.0)],
        Vec3::splat(0.5),
    );
    floor.lightmap_uvs = floor
        .positions
        .iter()
        .map(|q| Vec2::new(texel(q.x), texel(q.z)) / S as f32)
        .collect();
    // A 2 m wall along x = 0 facing the floor, its back to a sun at 45° behind it.
    let w = |y, z| Vec3::new(0.0, y, z);
    let mut wall = quad(
        2,
        [w(0.0, -1.0), w(2.0, -1.0), w(2.0, 5.0), w(0.0, 5.0)],
        Vec3::ONE,
    );
    wall.lightmap_uvs = vec![Vec2::ZERO; 4];
    let scene = BakeScene {
        meshes: vec![floor, wall],
        lights: vec![BakeLight {
            shape: LightShape::Directional {
                direction: Vec3::new(1.0, -1.0, 0.0).normalize(),
            },
            radiance: Vec3::splat(3.0),
            bakes_direct: true,
        }],
        sky: Vec3::splat(0.5).into(),
    };
    let settings = BakeSettings {
        texels_per_unit: 4.0,
        samples: 64,
        max_resolution: S,
        ..super::tests::settings()
    };
    let map = &bake(&scene, &settings)[0];
    assert_eq!(map.size, S);
    // Mean of column `x` over the rows clear of the floor's other borders.
    let column = |x: usize| {
        (2..12)
            .map(|y| map.texels[y * S as usize + x].x)
            .sum::<f32>()
            / 10.0
    };
    let (border, shade, sun) = (column(0), column(2), column(12));
    assert!(
        sun > 2.0 * shade,
        "the wall shades the floor near it: {sun} vs {shade}"
    );
    assert!(
        border <= shade * 1.1,
        "the border leaks light: {border} vs {shade}"
    );
    assert!(
        border >= shade * 0.4,
        "the border goes dark: {border} vs {shade}"
    );
}
