//! The `.blend` fixture through Blender (#1072). These need Blender 5+, so they
//! are skipped by default and run by CI's `blender` job (`--run-ignored only`).
//! Finding Blender is tested without it, with fakes (`locate`).

use crate::asset::convert::{converter_for, refresh, textures_dirs};
use crate::asset::gltf::import::import_from;
use crate::asset::gltf::uri;
use crate::asset::{ImportedAsset, MapRef};
use std::path::{Path, PathBuf};

mod drop;
mod fingerprint;
mod library_image;
mod linked;
#[cfg(unix)]
mod locate;
mod packed_formats;
#[cfg(unix)]
mod run;

pub(super) fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(crate::core::paths::native("tests/fixtures/models"))
        .join(name)
}

/// `textured.blend` converted fresh into `out`, then imported as an import would.
fn import(out: &Path) -> ImportedAsset {
    let source = fixture("textured.blend");
    let converter = converter_for(&source).expect("a .blend converts");
    refresh(converter, &source, out).expect("the fixture converts");
    import_from(out, Some(&source.to_string_lossy())).expect("its glTF imports")
}

/// The colour of the first pixel of the base colour map material `material` of
/// the `.glb` at `out` samples, read from the file its glTF image names.
pub(super) fn pixel(out: &Path, material: &str) -> [u8; 4] {
    let doc = gltf::Gltf::open(out).expect("the cached .glb reads");
    let m = doc.materials().find(|m| m.name() == Some(material));
    let info = m.and_then(|m| m.pbr_metallic_roughness().base_color_texture());
    let image = info.expect(material).texture().source();
    let gltf::image::Source::Uri { uri, .. } = image.source() else {
        panic!("{material}'s image is a file beside the .glb");
    };
    let file = out.parent().unwrap().join(uri::decode(uri).expect("a URI"));
    let decoded = image::open(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    decoded.to_rgba8().get_pixel(0, 0).0
}

pub(super) fn base_color_map(asset: &ImportedAsset, material: &str) -> String {
    let m = asset.materials.iter().find(|m| m.name == material);
    let map = m.and_then(|m| m.base_color_map.as_ref()).expect(material);
    map.as_str().to_string()
}

#[test]
// lint: allow(skip) needs Blender 5+; CI's `blender` job installs it and runs this
#[ignore]
fn a_blend_imports_to_a_gltf_with_its_meshes() {
    let asset = import(&crate::test_temp::dir().join("textured.blend.glb"));
    let mut meshes = asset.sub_mesh_ids();
    meshes.sort();
    assert_eq!(meshes, ["Cube", "Plane"]);
    let cube = asset.sub_meshes.iter().find(|m| m.id == "Cube").unwrap();
    assert_eq!(cube.indices.len(), 36, "twelve triangles");
}

#[test]
// lint: allow(skip) needs Blender 5+; CI's `blender` job installs it and runs this
#[ignore]
fn a_packed_image_is_extracted_flat_and_becomes_a_reference() {
    let out = crate::test_temp::dir().join("textured.blend.glb");
    let asset = import(&out);
    let source = fixture("textured.blend");
    let map = base_color_map(&asset, "Packed");
    assert_eq!(map, format!("{}#tex/Checker", source.display()));
    let [dir] = textures_dirs(&out).try_into().expect("one textures folder");
    let files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap())
        .collect();
    assert_eq!(files.len(), 1, "flat: one file, no subfolder");
    assert!(files[0].file_type().unwrap().is_file());
    // Its name says `.jpg`, its bytes are a PNG: the bytes win.
    assert_eq!(files[0].file_name(), "Checker.png");
    assert!(std::fs::read(files[0].path())
        .unwrap()
        .starts_with(b"\x89PNG"));
}

#[test]
// lint: allow(skip) needs Blender 5+; CI's `blender` job installs it and runs this
#[ignore]
fn an_image_on_disk_stays_a_plain_path_and_is_a_dependency() {
    let out = crate::test_temp::dir().join("textured.blend.glb");
    let asset = import(&out);
    let map = base_color_map(&asset, "External");
    let bricks = fixture("textured_bricks.png");
    assert_eq!(MapRef::new(&map).source(), None, "not a reference: {map}");
    assert_eq!(std::fs::canonicalize(&map).unwrap(), bricks);
    let stamp = std::fs::read_to_string(super::super::record::stamp_path(&out)).unwrap();
    assert!(stamp.contains("textured_bricks.png"), "{stamp}");
    let tool = super::identify().expect("a Blender");
    assert!(
        stamp.lines().next().unwrap().ends_with(&format!(" {tool}")),
        "{stamp}"
    );
}

#[test]
// lint: allow(skip) needs Blender 5+; CI's `blender` job installs it and runs this
#[ignore]
fn an_unused_missing_image_is_neither_a_dependency_nor_a_skip() {
    let out = crate::test_temp::dir().join("textured.blend.glb");
    import(&out);
    let stamp = std::fs::read_to_string(super::super::record::stamp_path(&out)).unwrap();
    assert!(!stamp.contains("no_such_image"), "{stamp}");
    assert!(!stamp.contains("skipped"), "{stamp}");
}
