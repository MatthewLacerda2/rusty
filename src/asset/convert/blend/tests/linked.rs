//! An image's identity through a linked library (#1072): `linked.blend` and the
//! file it links, `linked_lib.blend`, each pack an image named `Wood`. Needs
//! Blender 5+, so skipped by default (see `tests`).

use super::{base_color_map, fixture};
use crate::asset::convert::{converter_for, refresh};
use crate::asset::gltf::import::import_from;
use std::path::Path;

/// The colour of the first pixel of the image the `.glb` at `out` names `name`.
fn pixel(out: &Path, name: &str) -> [u8; 4] {
    let doc = gltf::Gltf::open(out).expect("the cached .glb reads");
    let image = doc.images().find(|i| i.name() == Some(name)).expect(name);
    let gltf::image::Source::Uri { uri, .. } = image.source() else {
        panic!("{name} is a file beside the .glb");
    };
    let file = out.parent().unwrap().join(uri);
    image::open(&file)
        .expect("decodes")
        .to_rgba8()
        .get_pixel(0, 0)
        .0
}

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
    assert_eq!(pixel(&out, "Wood"), [255, 0, 0, 255], "red, the local one");
    assert_eq!(
        pixel(&out, "Wood.1"),
        [0, 0, 255, 255],
        "blue, the linked one"
    );
    let stamp = std::fs::read_to_string(super::super::super::record::stamp_path(&out)).unwrap();
    assert!(
        stamp.contains("linked_lib.blend"),
        "the library is a dependency: {stamp}"
    );
}
