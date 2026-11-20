//! Cooking (#2332): the threaded steps give the single-threaded asset, bit for bit.

use super::fixture;
use super::par::with_threads;
use crate::{clusters, import_file};

#[test]
fn a_threaded_import_is_the_single_threaded_one_bit_for_bit() {
    let path = fixture::model("threads");
    let serial = with_threads(1, || import_file(&path).unwrap());
    assert_eq!(serial.sub_meshes.len(), 3, "the fixture's three objects");
    let grid = &serial.sub_meshes[0];
    assert!(grid.indices.len() / 3 >= clusters::MIN_TRIANGLES);
    assert!(grid.clusters.is_some(), "the grid is clustered");
    assert!(
        grid.vertices.iter().any(|v| v.lightmap_uv != [0.0; 2]),
        "the grid is unwrapped"
    );
    assert!(
        grid.vertices
            .iter()
            .any(|v| v.tangent != [1.0, 0.0, 0.0, 1.0]),
        "its tangents are generated"
    );
    for threads in [2, 3, 8] {
        let threaded = with_threads(threads, || import_file(&path).unwrap());
        // `PartialEq` compares floats by value; the printed form shows a -0 too.
        assert_eq!(threaded, serial, "{threads} threads");
        assert_eq!(format!("{threaded:?}"), format!("{serial:?}"));
    }
}

#[test]
fn a_threaded_import_is_still_one_parse() {
    let path = fixture::model("one_parse");
    let before = crate::import_cache::parses();
    with_threads(4, || crate::imported(&path).unwrap());
    with_threads(4, || crate::imported(&path).unwrap());
    assert_eq!(crate::import_cache::parses() - before, 1);
}

#[test]
fn a_cooked_sub_mesh_is_laid_only_on_the_parse_it_was_cooked_from() {
    let path = fixture::model("parse_digest");
    super::with_writes(true, || import_file(&path).unwrap());
    let bytes = std::fs::read(fixture::cooked_file(&path)).unwrap();
    let stamp = super::stamp::stamp(&path, &crate::settings_of(&path)).unwrap();
    let cooked = super::file::parse(&bytes, stamp).unwrap();
    let parse = || {
        let missing = crate::tangents::Missing::Placeholder;
        crate::obj_import::import_as(&path, missing)
            .unwrap()
            .sub_meshes[1]
            .clone()
    };
    assert_eq!(cooked[1].clone().apply(&mut parse()), Ok(()));
    // What a parser fix would change, the positions and counts kept.
    let mut uv = parse();
    let v = &mut uv.vertices.edit()[0];
    v.tex_coords[1] = 1.0 - v.tex_coords[1];
    let mut winding = parse();
    winding.indices.edit().swap(0, 1);
    let mut normal = parse();
    normal.vertices.edit()[2].normal[0] += 0.5;
    for mut changed in [uv, winding, normal] {
        let before = changed.clone();
        let e = cooked[1].clone().apply(&mut changed).unwrap_err();
        assert!(e.contains("another parse"), "{e}");
        assert_eq!(changed, before, "left as it was");
    }
}

#[test]
fn an_obj_whose_mtllib_is_missing_cooks_to_disk_once() {
    let path = fixture::model("missing_mtllib");
    let text = std::fs::read_to_string(&path).unwrap();
    // `tobj` reads the rest of the line as one name: one missing file.
    std::fs::write(&path, format!("mtllib a.mtl b.mtl\n{text}")).unwrap();
    let written = || {
        let before = super::written();
        super::with_writes(true, || import_file(&path).unwrap());
        super::written() - before
    };
    assert_eq!(written(), 1, "cooked and written");
    assert_eq!(written(), 0, "then read");
    let mtl = path.parent().unwrap().join("a.mtl b.mtl");
    std::fs::write(mtl, "newmtl M\nKd 1 0 0\n").unwrap();
    assert_eq!(written(), 1, "the side file appearing cooks it again");
}

#[test]
fn a_parse_digest_tells_a_skinned_sub_mesh_from_a_static_one() {
    // The unwrap and the clusters skip a skinned mesh, so values cooked from one
    // never fit the other, however alike their vertices.
    let path = fixture::model("digest_skin");
    let parsed = import_file(&path).unwrap();
    let mut sub = parsed.sub_meshes[1].clone();
    let bare = super::sub::digest(&sub);
    sub.skin = Some(crate::mesh_data::SkinData::default());
    assert_ne!(super::sub::digest(&sub), bare);
}
