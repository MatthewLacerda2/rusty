//! The mesh-map bake at its edges (#1849): sizes, broken meshes, overlapping UVs.

use glam::{Vec2, Vec3};

use super::fixtures::{bake_all, cell, l_shape, quad};
use super::{MeshMap, MeshSource, MAX_RESOLUTION};

fn err(mesh: &MeshSource, size: u32) -> String {
    bake_all(mesh, &[MeshMap::Ao], size).unwrap_err()
}

#[test]
fn a_one_texel_map_bakes_every_map() {
    let maps = bake_all(&l_shape(), &MeshMap::ALL, 1).unwrap();
    assert_eq!(maps.len(), MeshMap::ALL.len());
    assert!(maps
        .iter()
        .all(|(_, img)| img.width() == 1 && img.height() == 1));
}

#[test]
fn a_resolution_out_of_range_is_refused() {
    assert!(err(&l_shape(), 0).contains("resolution 0"));
    assert!(err(&l_shape(), MAX_RESOLUTION + 1).contains("outside"));
}

#[test]
fn a_broken_mesh_is_an_error_never_a_panic() {
    assert!(err(&MeshSource::default(), 8).contains("no triangles"));
    let flat_uv = MeshSource {
        uvs: vec![Vec2::ZERO; l_shape().uvs.len()],
        ..l_shape()
    };
    assert!(err(&flat_uv, 8).contains("no UV0"));
    let mut nan = l_shape();
    nan.positions[0] = Vec3::NAN;
    assert!(err(&nan, 8).contains("non-finite"));
    let mut past = l_shape();
    past.indices[0] = 99;
    assert!(err(&past, 8).contains("index past"));
    let mut outside = l_shape();
    outside.uvs.iter_mut().for_each(|u| *u += Vec2::splat(2.0));
    assert!(err(&outside, 8).contains("0..1"));
}

#[test]
fn overlapping_uvs_give_a_texel_to_the_first_triangle() {
    // The wall drawn over the floor's UV cell: the floor (first) keeps it.
    let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    let mut m = MeshSource::default();
    quad(
        &mut m,
        [v(0., 0., 0.), v(0., 0., 1.), v(1., 0., 1.), v(1., 0., 0.)],
        cell(0),
        0,
    );
    quad(
        &mut m,
        [v(0., 0., 0.), v(0., 1., 0.), v(0., 1., 1.), v(0., 0., 1.)],
        cell(0),
        0,
    );
    let n = bake_all(&m, &[MeshMap::WorldNormal], 32)
        .unwrap()
        .remove(0)
        .1;
    let (lo, hi) = cell(0);
    let p = ((lo + hi) * 0.5 * 32.0).floor();
    let texel = n.pixels()[p.y as usize * 32 + p.x as usize];
    assert!((texel[1] - 1.0).abs() < 1e-3, "{texel:?}");
}

#[test]
fn a_map_asked_twice_bakes_once_and_unknown_names_are_listed() {
    let maps = bake_all(&l_shape(), &[MeshMap::Id, MeshMap::Id], 4).unwrap();
    assert_eq!(maps.len(), 1);
    let e = MeshMap::parse("cavity").unwrap_err();
    assert!(
        e.contains("\"cavity\"") && e.contains("world_normal"),
        "{e}"
    );
    assert_eq!(MeshMap::parse("AO"), Ok(MeshMap::Ao));
}
