//! Which geometry a texel reads (#1849's review): only its own connected surface's
//! creases, no crease between a face and its reversed copy, corners welded through
//! `-0.0`, rays above the face's own plane, and the ID map keyed on the material.

use glam::{Vec2, Vec3};

use super::fixtures::{bake_all, cell, cube, quad};
use super::{MeshMap, MeshSource};
use rusty_procgen::image_buf::Image;

const S: u32 = 96;

fn one(mesh: &MeshSource, map: MeshMap) -> Image {
    bake_all(mesh, &[map], S).unwrap().remove(0).1
}

/// Every texel centre strictly inside the UV rectangle `r`, two texels in.
fn inside(img: &Image, (lo, hi): (Vec2, Vec2)) -> impl Iterator<Item = f32> + '_ {
    let (a, b) = ((lo * S as f32).ceil() + 2.0, (hi * S as f32).floor() - 2.0);
    (a.y as usize..b.y as usize)
        .flat_map(move |y| (a.x as usize..b.x as usize).map(move |x| (x, y)))
        .map(move |(x, y)| img.pixels()[y * S as usize + x][0])
}

const FLOOR: [Vec3; 4] = [
    Vec3::new(0., 0., 0.),
    Vec3::new(0., 0., 1.),
    Vec3::new(1., 0., 1.),
    Vec3::new(1., 0., 0.),
];

#[test]
fn a_box_on_a_separate_plane_leaves_the_plane_flat() {
    let mut m = cube();
    m.positions
        .iter_mut()
        .for_each(|p| *p = *p * 0.2 + Vec3::new(0.4, 0.0, 0.4));
    m.uvs.iter_mut().for_each(|u| *u *= Vec2::new(1.0, 0.5));
    let plane = (Vec2::new(0.05, 0.55), Vec2::new(0.95, 0.95));
    quad(&mut m, FLOOR, plane, 0);
    let img = one(&m, MeshMap::Curvature);
    assert!(inside(&img, plane).all(|v| (v - 0.5).abs() < 1e-3));
    // The box's own edges still light: its top face's border is bright.
    let (lo, _) = cell(0);
    let edge = ((lo * Vec2::new(1.0, 0.5) * S as f32).ceil() + 0.5).as_uvec2();
    assert!(img.pixels()[(edge.y * S + edge.x + 4) as usize][0] > 0.8);
}

#[test]
fn a_double_sided_face_has_no_crease_at_its_outline() {
    let mut m = MeshSource::default();
    quad(&mut m, FLOOR, cell(0), 0);
    let [a, b, c, d] = FLOOR;
    quad(&mut m, [a, d, c, b], cell(1), 0);
    let img = one(&m, MeshMap::Curvature);
    for k in 0..2 {
        assert!(inside(&img, cell(k)).all(|v| (v - 0.5).abs() < 1e-3), "{k}");
    }
}

#[test]
fn a_negative_zero_corner_still_welds() {
    let mut m = cube();
    // The -X face (cell 3, vertices 12..16) written with x = -0.0.
    m.positions[12..16].iter_mut().for_each(|p| p.x = -0.0);
    let img = one(&m, MeshMap::Curvature);
    let (lo, hi) = cell(3);
    let y = ((lo.y + hi.y) * 0.5 * S as f32) as u32;
    let x = (lo.x * S as f32).ceil() as u32;
    assert!(img.pixels()[(y * S + x) as usize][0] > 0.8);
}

#[test]
fn ao_never_casts_under_the_faces_own_plane() {
    // A floor whose normals lean hard towards +X, over a second floor below it.
    let mut m = MeshSource::default();
    quad(&mut m, FLOOR, cell(0), 0);
    m.normals
        .iter_mut()
        .for_each(|n| *n = Vec3::new(1.0, 0.3, 0.0).normalize());
    quad(&mut m, FLOOR.map(|p| p - Vec3::Y * 0.2), cell(1), 0);
    let img = one(&m, MeshMap::Ao);
    assert!(inside(&img, cell(0)).all(|v| v > 0.999), "the top is open");
}

#[test]
fn the_id_map_colours_by_material_not_by_run() {
    // Metal, Rubber, Metal: three runs, two materials.
    let mut m = MeshSource::default();
    for (k, material) in [0, 1, 0].into_iter().enumerate() {
        let shift = Vec3::X * 2.0 * k as f32;
        quad(&mut m, FLOOR.map(|p| p + shift), cell(k), material);
    }
    assert_eq!(m.runs.len(), 3);
    let img = one(&m, MeshMap::Id);
    let colour = |k: usize| {
        let c = (cell(k).0 + cell(k).1) * 0.5 * S as f32;
        img.pixels()[c.y as usize * S as usize + c.x as usize]
    };
    assert_eq!(colour(0), colour(2));
    assert_ne!(colour(0), colour(1));
}
