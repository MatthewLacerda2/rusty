//! An image's identity through a linked library (#1072): `linked.blend` and the
//! file it links, `linked_lib.blend`, each pack an image named `Wood`. Needs
//! Blender 5+, so skipped by default (see `tests`).

use super::{base_color_map, fixture, pixel};
use crate::asset::convert::{converter_for, refresh};
use crate::asset::gltf::import::import_from;

#[test]
// lint: allow(skip) needs Blender 5+; CI's `blender` job installs it and runs this
#[ignore]
fn a_local_and_a_linked_image_of_one_name_keep_their_own_pixels() {
    let source = fixture("linked.blend");
    let out = crate::test_temp::dir().join("linked.blend.glb");
    refresh(converter_for(&source).unwrap(), &source, &out).expect("converts");
    let asset = import_from(&out, Some(&source.to_string_lossy())).expect("imports");
    let named = |identity: &str| format!("{}#tex/{identity}", source.display());
    assert_eq!(
        base_color_map(&asset, "Local"),
        named("Wood"),
        "the first keeps it"
    );
    assert_eq!(base_color_map(&asset, "Linked"), named("Wood.1"));
    assert_eq!(pixel(&out, "Local"), [255, 0, 0, 255], "red, the local one");
    assert_eq!(
        pixel(&out, "Linked"),
        [0, 0, 255, 255],
        "blue, the linked one"
    );
    let stamp = std::fs::read_to_string(super::super::super::record::stamp_path(&out)).unwrap();
    assert!(
        stamp.contains("linked_lib.blend"),
        "the library is a dependency: {stamp}"
    );
}
