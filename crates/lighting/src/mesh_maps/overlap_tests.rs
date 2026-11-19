//! A texel triangles share in UV0 bakes the most open of them, in every map (#2310):
//! a panel 6 mm in front of a wall, both mapped to one UV square.

use glam::{Vec2, Vec3};

use super::fixtures::{bake_with, cube, quad};
use super::{MeshMap, MeshSource, Settings};
use crate::lightmap::rasterize_overlaps;
use rusty_procgen::image_buf::Image;

const S: u32 = 32;
const UV: (Vec2, Vec2) = (Vec2::splat(0.25), Vec2::splat(0.75));

/// The unit square at depth `z`, facing +Z, in [`UV`].
fn square(m: &mut MeshSource, z: f32) {
    let v = |x: f32, y: f32| Vec3::new(x, y, z);
    quad(m, [v(0., 0.), v(1., 0.), v(1., 1.), v(0., 1.)], UV, 0);
}

/// The wall (z 0) and the panel 6 mm in front of it, hiding it, in either order;
/// the wall's normals lean, so its `world_normal` texels are told from the panel's.
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

fn bake(mesh: &MeshSource, maps: &[MeshMap]) -> Vec<(MeshMap, Image)> {
    bake_with(mesh, maps, &Settings::new(S)).unwrap()
}

/// Every texel the UV square covers.
fn shared(img: &Image) -> impl Iterator<Item = [f32; 4]> + '_ {
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
    // The panel's: its flat +Z normal, at the front of the bounds.
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
    let (a, b) = (bake(&mesh, &all), bake(&mesh, &all));
    for ((map, x), (_, y)) in a.iter().zip(&b) {
        assert_eq!(x.pixels(), y.pixels(), "{}", map.name());
    }
}

#[test]
fn a_layout_without_overlap_has_no_claimant_even_where_its_edges_cross_texel_centres() {
    // A grid of 64 quads over the whole square: every shared diagonal and edge
    // passes through texel centres or corners.
    let mut grid = MeshSource::default();
    for k in 0..64 {
        let (x, y) = ((k % 8) as f32 / 8.0, (k / 8) as f32 / 8.0);
        let v = |dx: f32, dy: f32| Vec3::new(x + dx, y + dy, 0.0);
        let c = [v(0., 0.), v(0.125, 0.), v(0.125, 0.125), v(0., 0.125)];
        quad(&mut grid, c, (Vec2::new(x, y), Vec2::new(x, y) + 0.125), 0);
    }
    for (name, mesh) in [("grid", grid), ("cube", cube())] {
        for size in [S, 37] {
            let (points, others) = rasterize_overlaps(&mesh.surface(), size, size);
            assert!(!points.is_empty());
            assert!(
                others.is_empty(),
                "{name} at {size}: {} claimants",
                others.len()
            );
        }
    }
}
