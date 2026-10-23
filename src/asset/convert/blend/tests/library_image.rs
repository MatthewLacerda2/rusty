//! A linked library's image on disk (#1072): `library_image/direct.blend` packs a
//! red image named `Wood` and links a crate from `sub2/lib.blend`, whose material
//! samples `sub2/tex.png`, a green file on disk also named `Wood`. Needs Blender
//! 5+, so skipped by default (see `tests`).

use super::{base_color_map, fixture, pixel};
use crate::asset::convert::{converter_for, record, refresh};
use crate::asset::gltf::import::import_from;
use crate::asset::{ImportedAsset, MapRef};
use std::path::{Path, PathBuf};

/// The fixture copied into a folder of its own, with `decoy` (a white `tex.png`
/// beside `direct.blend`, where the exporter alone would look) or without.
fn copied(decoy: bool) -> PathBuf {
    let (from, dir) = (fixture("library_image"), crate::test_temp::dir());
    std::fs::create_dir_all(dir.join("sub2")).unwrap();
    for file in ["direct.blend", "sub2/lib.blend", "sub2/tex.png"] {
        std::fs::copy(from.join(file), dir.join(file)).unwrap();
    }
    if decoy {
        std::fs::copy(fixture("textured_bricks.png"), dir.join("tex.png")).unwrap();
    }
    dir
}

fn import(dir: &Path) -> (ImportedAsset, PathBuf) {
    let (source, out) = (dir.join("direct.blend"), dir.join("direct.blend.glb"));
    refresh(converter_for(&source).unwrap(), &source, &out).expect("converts");
    let asset = import_from(&out, Some(&source.to_string_lossy())).expect("imports");
    (asset, out)
}

fn the_library_image_is_its_own(decoy: bool) {
    let dir = copied(decoy);
    let (asset, out) = import(&dir);
    let source = dir.join("direct.blend");
    assert_eq!(
        base_color_map(&asset, "Local"),
        format!("{}#tex/Wood", source.display()),
        "the packed one is made, under its name"
    );
    assert_eq!(
        pixel(&out, "Local"),
        [255, 0, 0, 255],
        "red, the packed one"
    );
    let linked = base_color_map(&asset, "Linked");
    assert_eq!(
        MapRef::new(&linked).source(),
        None,
        "a plain path: {linked}"
    );
    let tex = std::fs::canonicalize(dir.join("sub2/tex.png")).unwrap();
    assert_eq!(std::fs::canonicalize(&linked).unwrap(), tex);
    assert_eq!(
        pixel(&out, "Linked"),
        [0, 255, 0, 255],
        "green, the library's"
    );
    let stamp = std::fs::read_to_string(record::stamp_path(&out)).unwrap();
    assert!(stamp.contains("sub2/tex.png"), "a dependency: {stamp}");
    assert!(!stamp.contains("skipped"), "{stamp}");
}

#[test]
// lint: allow(skip) needs Blender 5+; CI's `blender` job installs it and runs this
#[ignore]
fn a_library_image_on_disk_is_found_beside_the_library() {
    the_library_image_is_its_own(false);
}

#[test]
// lint: allow(skip) needs Blender 5+; CI's `blender` job installs it and runs this
#[ignore]
fn a_decoy_beside_the_main_file_is_not_the_library_image() {
    the_library_image_is_its_own(true);
}
