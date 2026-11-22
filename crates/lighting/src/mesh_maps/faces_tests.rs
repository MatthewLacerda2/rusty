use glam::{Vec2, Vec3};

use super::fixtures::{bake_with, quad};
use super::{Faces, MeshMap, MeshSource, Settings};
use rusty_procgen::image_buf::Image;

const S: u32 = 32;

/// The issue's smallest case: two 1 m quads 5 cm apart, facing each other, each its own material
/// and each filling UV0's unit square.
fn facing() -> MeshSource {
    let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    let whole = (Vec2::ZERO, Vec2::ONE);
    let mut m = MeshSource::default();
    let up = [v(0., 0., 0.), v(1., 0., 0.), v(1., 1., 0.), v(0., 1., 0.)];
    let down = [
        v(0., 0., 0.05),
        v(0., 1., 0.05),
        v(1., 1., 0.05),
        v(1., 0., 0.05),
    ];
    quad(&mut m, up, whole, 0);
    quad(&mut m, down, whole, 1);
    m
}

fn faces(materials: Option<&[usize]>, ignore: &[usize]) -> Settings {
    let mut s = Settings::new(S);
    s.faces = Faces {
        materials: materials.map(<[usize]>::to_vec),
        ignore_occluders: ignore.to_vec(),
    };
    s
}

fn maps(settings: &Settings, maps: &[MeshMap]) -> Vec<Image> {
    let baked = bake_with(&facing(), maps, settings).unwrap();
    baked.into_iter().map(|(_, img)| img).collect()
}

fn mean_red(img: &Image) -> f32 {
    img.pixels().iter().map(|p| p[0]).sum::<f32>() / img.pixels().len() as f32
}

#[test]
fn each_material_bakes_its_own_faces_alone_where_they_share_uv0() {
    let pick = [MeshMap::WorldNormal, MeshMap::Coverage];
    for (material, blue) in [(0, 1.0), (1, 0.0)] {
        let img = maps(&faces(Some(&[material]), &[]), &pick);
        let covered: Vec<_> = (img[1].pixels().iter()).map(|p| p[0] == 1.0).collect();
        assert!(covered.iter().all(|&c| c), "material {material} covers UV0");
        for p in img[0].pixels() {
            assert_eq!(p[2], blue, "material {material}'s normal on every texel");
        }
    }
}

#[test]
fn the_other_material_still_occludes_unless_it_is_ignored() {
    let shut = mean_red(&maps(&faces(Some(&[0]), &[]), &[MeshMap::Ao])[0]);
    let open = mean_red(&maps(&faces(Some(&[0]), &[1]), &[MeshMap::Ao])[0]);
    assert!(shut < 0.3, "a pane 5 cm above shades the quad: {shut}");
    assert_eq!(open, 1.0, "an ignored pane blocks no ray");
}

#[test]
fn an_ignored_material_stops_no_thickness_ray() {
    let mut mesh = facing();
    for p in &mut mesh.positions[4..] {
        p.z = -p.z;
    }
    let thick = |ignore: &[usize]| {
        let one = bake_with(&mesh, &[MeshMap::Thickness], &faces(Some(&[0]), ignore));
        mean_red(&one.unwrap()[0].1)
    };
    assert!(
        thick(&[]) < 0.3,
        "a pane 5 cm behind reads thin: {}",
        thick(&[])
    );
    assert!(
        thick(&[1]) > 0.999,
        "an ignored one is not there: {}",
        thick(&[1])
    );
}

#[test]
fn an_ignored_material_writes_nothing_unless_it_is_asked_for() {
    let img = maps(&faces(None, &[1]), &[MeshMap::WorldNormal, MeshMap::Ao]);
    assert!(
        img[0].pixels().iter().all(|p| p[2] == 1.0),
        "only the opaque quad writes"
    );
    assert_eq!(mean_red(&img[1]), 1.0, "and it bakes open");
    let own = maps(&faces(Some(&[1]), &[1]), &[MeshMap::WorldNormal]);
    assert!(
        own[0].pixels().iter().all(|p| p[2] == 0.0),
        "the pane's own atlas"
    );
}

#[test]
fn a_material_without_faces_on_uv0_is_an_error() {
    let e = bake_with(&facing(), &[MeshMap::Ao], &faces(Some(&[7]), &[])).unwrap_err();
    assert!(e.contains("no triangle of the asked materials"), "{e}");
}

#[test]
fn ignoring_every_material_is_an_error_naming_ignore_occluders() {
    let e = bake_with(&facing(), &[MeshMap::Ao], &faces(None, &[0, 1])).unwrap_err();
    assert!(e.contains("outside ignore_occluders' materials"), "{e}");
}
