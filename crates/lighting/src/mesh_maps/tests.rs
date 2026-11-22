use glam::{Vec2, Vec3};

use super::fixtures::{bake_all, cell, cube, l_shape};
use super::{MeshMap, MeshSource};
use rusty_procgen::image_buf::Image;

const S: u32 = 96;

fn one(mesh: &MeshSource, map: MeshMap) -> Image {
    bake_all(mesh, &[map], S).unwrap().remove(0).1
}

fn at(img: &Image, uv: Vec2) -> [f32; 4] {
    let p = (uv * S as f32).floor();
    img.pixels()[p.y as usize * S as usize + p.x as usize]
}

fn first_in(lo: f32) -> f32 {
    ((lo * S as f32).floor() + 1.5) / S as f32
}
fn last_in(hi: f32) -> f32 {
    ((hi * S as f32).ceil() - 1.5) / S as f32
}

fn mid(k: usize) -> Vec2 {
    let (lo, hi) = cell(k);
    (lo + hi) * 0.5
}

#[test]
fn a_cubes_curvature_is_bright_along_its_uv_mapped_edges_and_flat_inside() {
    let img = one(&cube(), MeshMap::Curvature);
    for k in 0..6 {
        let (lo, hi) = cell(k);
        let centre = at(&img, mid(k))[0];
        assert!((centre - 0.5).abs() < 1e-3, "face {k} centre {centre}");
        let m = mid(k);
        for edge in [
            Vec2::new(first_in(lo.x), m.y),
            Vec2::new(last_in(hi.x), m.y),
            Vec2::new(m.x, first_in(lo.y)),
            Vec2::new(m.x, last_in(hi.y)),
        ] {
            let v = at(&img, edge)[0];
            assert!(v > 0.8, "face {k} edge texel {edge} reads {v}");
        }
        let six = Vec2::new(lo.x + 6.5 / S as f32, m.y);
        assert!((at(&img, six)[0] - 0.5).abs() < 1e-3, "face {k} six in");
    }
}

#[test]
fn an_ls_inner_edge_is_dark_in_curvature_and_in_ao() {
    let mesh = l_shape();
    let maps = bake_all(&mesh, &[MeshMap::Curvature, MeshMap::Ao], S).unwrap();
    let (curv, ao) = (&maps[0].1, &maps[1].1);
    let (lo, hi) = cell(0);
    let near = Vec2::new(mid(0).x, first_in(lo.y));
    let far = Vec2::new(mid(0).x, last_in(hi.y));
    assert!(at(curv, near)[0] < 0.2, "{:?}", at(curv, near));
    assert!(
        (at(curv, far)[0] - 0.5).abs() < 1e-3,
        "an open border is no crease"
    );
    let (dark, open) = (at(ao, near)[0], at(ao, far)[0]);
    assert!(
        dark < 0.7 && open > dark + 0.2,
        "corner {dark}, far side {open}"
    );
}

#[test]
fn rebaking_gives_the_same_bytes() {
    let all = MeshMap::ALL;
    let a = bake_all(&l_shape(), &all, 32).unwrap();
    let b = bake_all(&l_shape(), &all, 32).unwrap();
    assert_eq!(a, b);
}

#[test]
fn normal_and_position_read_the_face_they_sit_on() {
    let (n, p) = (
        one(&cube(), MeshMap::WorldNormal),
        one(&cube(), MeshMap::Position),
    );
    let close = |a: [f32; 4], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 0.02);
    assert!(
        close(at(&n, mid(0)), [0.5, 1.0, 0.5]),
        "{:?}",
        at(&n, mid(0))
    );
    assert!(
        close(at(&p, mid(0)), [0.5, 1.0, 0.5]),
        "{:?}",
        at(&p, mid(0))
    );
    assert!(close(at(&n, mid(3)), [0.0, 0.5, 0.5]), "the -X face");
}

#[test]
fn each_material_gets_its_own_id_colour_and_the_margin_copies_one() {
    let img = one(&l_shape(), MeshMap::Id);
    let (floor, wall) = (at(&img, mid(0)), at(&img, mid(1)));
    assert_ne!(floor, wall);
    let gutter = at(&img, Vec2::new(1.0 / 3.0, mid(0).y));
    assert!(gutter == floor || gutter == wall, "{gutter:?}");
    assert_eq!(at(&img, Vec2::new(0.99, 0.99)), [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn a_thin_slab_reads_thinner_than_a_cube() {
    let thin = MeshSource {
        positions: cube()
            .positions
            .iter()
            .map(|p| *p * Vec3::new(1.0, 0.05, 1.0))
            .collect(),
        ..cube()
    };
    let (thick, slab) = (
        one(&cube(), MeshMap::Thickness),
        one(&thin, MeshMap::Thickness),
    );
    let (t, s) = (at(&thick, mid(0))[0], at(&slab, mid(0))[0]);
    assert!(t > 0.4 && s < 0.1, "cube {t}, slab {s}");
}

#[test]
fn one_margin_pass_fills_the_ring_around_a_texel_and_no_farther() {
    let size = rusty_procgen::image_buf::Size {
        width: 7,
        height: 4,
    };
    let (mut px, mut filled) = (vec![[0.0; 4]; 28], vec![false; 28]);
    (px[11], filled[11]) = ([1.0; 4], true);
    super::margin::grow(&mut px, &mut filled, size, 1, true);
    for (i, f) in filled.iter().enumerate() {
        let (x, y, near) = (i % 7, i / 7, (3..=5).contains(&(i % 7)) && i / 7 <= 2);
        assert_eq!(
            (*f, px[i][0]),
            (near, f32::from(u8::from(near))),
            "({x}, {y})"
        );
    }
}
