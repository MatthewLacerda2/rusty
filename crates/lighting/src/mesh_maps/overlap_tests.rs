use glam::{Vec2, Vec3};

use super::fixtures::{bake_with, cube, quad};
use super::{MeshMap, MeshSource, Settings};
use crate::lightmap::rasterize_claims;
use rusty_procgen::image_buf::{Image, Size};

pub(super) const S: u32 = 32;
pub(super) const UV: (Vec2, Vec2) = (Vec2::splat(0.25), Vec2::splat(0.75));

pub(super) fn square(m: &mut MeshSource, z: f32) {
    let v = |x: f32, y: f32| Vec3::new(x, y, z);
    quad(m, [v(0., 0.), v(1., 0.), v(1., 1.), v(0., 1.)], UV, 0);
}

fn wall_and_panel(wall_first: bool) -> MeshSource {
    let mut m = MeshSource::default();
    let lean = Vec3::new(0.0, 0.6, 0.8);
    for wall in [wall_first, !wall_first] {
        let from = m.positions.len();
        square(&mut m, if wall { 0.0 } else { 0.006 });
        if wall {
            m.normals[from..].fill(lean);
        }
    }
    m
}

pub(super) fn bake(mesh: &MeshSource, maps: &[MeshMap]) -> Vec<(MeshMap, Image)> {
    bake_with(mesh, maps, &Settings::new(S)).unwrap()
}

pub(super) fn shared(img: &Image) -> impl Iterator<Item = [f32; 4]> + '_ {
    let (lo, hi) = ((UV.0.x * S as f32) as usize, (UV.1.x * S as f32) as usize);
    (lo..hi).flat_map(move |y| (lo..hi).map(move |x| img.pixels()[y * S as usize + x]))
}

#[test]
fn a_texel_the_panel_and_the_wall_share_bakes_the_open_panel_in_either_order() {
    for wall_first in [true, false] {
        let mesh = wall_and_panel(wall_first);
        let ao = &bake(&mesh, &[MeshMap::Ao])[0].1;
        for t in shared(ao) {
            assert_eq!(
                t[0], 1.0,
                "wall first: {wall_first}: a shared texel read {t:?}"
            );
        }
    }
}

#[test]
fn every_map_bakes_the_triangle_ao_picked_whether_or_not_ao_is_asked() {
    let mesh = wall_and_panel(true);
    let maps = [MeshMap::Ao, MeshMap::WorldNormal, MeshMap::Position];
    let with_ao = bake(&mesh, &maps);
    let without = bake(&mesh, &maps[1..]);
    assert_eq!(with_ao[1].1.pixels(), without[0].1.pixels(), "world_normal");
    assert_eq!(with_ao[2].1.pixels(), without[1].1.pixels(), "position");
    for (n, p) in shared(&with_ao[1].1).zip(shared(&with_ao[2].1)) {
        assert!(
            (n[2] - 1.0).abs() < 1e-6 && (n[1] - 0.5).abs() < 1e-6,
            "normal {n:?}"
        );
        assert!(p[2] > 0.999, "position {p:?}");
    }
}

#[test]
fn two_bakes_of_an_overlapping_model_are_identical() {
    let mesh = wall_and_panel(false);
    let all = MeshMap::ALL;
    for settings in [Settings::new(S), Settings::new(Size::new(4096, 8))] {
        let bake = || bake_with(&mesh, &all, &settings).unwrap();
        for ((map, x), (_, y)) in bake().iter().zip(&bake()) {
            assert_eq!(
                x.pixels(),
                y.pixels(),
                "{} at {:?}",
                map.name(),
                settings.size
            );
        }
    }
}

fn claimants(mesh: &MeshSource, width: u32, height: u32) -> usize {
    let mut n = 0;
    assert!(!rasterize_claims(&mesh.surface(), width, height, &mut |_| n += 1).is_empty());
    n
}

#[test]
fn a_layout_without_overlap_has_no_claimant_even_where_its_edges_cross_texel_centres() {
    let mut grid = MeshSource::default();
    for k in 0..64 {
        let (x, y) = ((k % 8) as f32 / 8.0, (k / 8) as f32 / 8.0);
        let v = |dx: f32, dy: f32| Vec3::new(x + dx, y + dy, 0.0);
        let c = [v(0., 0.), v(0.125, 0.), v(0.125, 0.125), v(0., 0.125)];
        quad(&mut grid, c, (Vec2::new(x, y), Vec2::new(x, y) + 0.125), 0);
    }
    for (name, mesh) in [("grid", grid), ("cube", cube())] {
        for (w, h) in [(S, S), (37, 37), (4096, 8), (8192, 4)] {
            assert_eq!(claimants(&mesh, w, h), 0, "{name} at {w}×{h}");
        }
    }
}

/// A slab's underside (the lower index) and top share UV0, over a floor: traced to the
/// diagonal the floor shades the underside and the open top takes the texels.
#[test]
fn a_short_ao_distance_changes_only_the_ao_map_where_uvs_overlap() {
    let mut m = MeshSource::default();
    let v = Vec3::new;
    let slab = (Vec2::ZERO, Vec2::splat(0.5));
    let under = [
        v(0., 0.5, 0.),
        v(1., 0.5, 0.),
        v(1., 0.5, 1.),
        v(0., 0.5, 1.),
    ];
    let top = [
        v(0., 0.55, 0.),
        v(0., 0.55, 1.),
        v(1., 0.55, 1.),
        v(1., 0.55, 0.),
    ];
    let floor = [
        v(-2., 0., -2.),
        v(-2., 0., 3.),
        v(3., 0., 3.),
        v(3., 0., -2.),
    ];
    quad(&mut m, under, slab, 0);
    quad(&mut m, top, slab, 0);
    quad(&mut m, floor, (Vec2::splat(0.6), Vec2::splat(0.9)), 0);
    let all = MeshMap::ALL;
    let bake = |ao_distance| {
        let settings = Settings {
            ao_distance,
            ..Settings::new(S)
        };
        bake_with(&m, &all, &settings).unwrap()
    };
    let (whole, short) = (bake(None), bake(Some(0.05)));
    for ((map, x), (_, y)) in whole.iter().zip(&short) {
        if *map != MeshMap::Ao {
            assert_eq!(x.pixels(), y.pixels(), "{}", map.name());
        }
    }
    let normal = &short
        .iter()
        .find(|(m, _)| *m == MeshMap::WorldNormal)
        .unwrap()
        .1;
    let up = normal.pixels()[(S / 4 * S + S / 4) as usize];
    assert!(
        up[1] > 0.999,
        "the shared texel bakes the top's normal, read {up:?}"
    );
}
