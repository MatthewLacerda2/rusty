//! The `.blend` fixture through Blender (#1072). These need Blender 5+, so they
//! are skipped by default and run by CI's `blender` job (`--run-ignored only`).
//! Finding Blender is tested without it, with fakes (`locate`).

use crate::asset::convert::{converter_for, refresh, textures_dirs};
use crate::asset::gltf::import::import_from;
use crate::asset::{ImportedAsset, MapRef};
use std::path::{Path, PathBuf};

#[cfg(unix)]
mod locate;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/models")
        .join(name)
}

/// `textured.blend` converted fresh into `out`, then imported as an import would.
fn import(out: &Path) -> ImportedAsset {
    let source = fixture("textured.blend");
    let converter = converter_for(&source).expect("a .blend converts");
    refresh(converter, &source, out).expect("the fixture converts");
    import_from(out, Some(&source.to_string_lossy())).expect("its glTF imports")
}

fn base_color_map(asset: &ImportedAsset, material: &str) -> String {
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
