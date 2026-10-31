//! A glTF material's `doubleSided` on import (#1615): honoured for an authored
//! `.gltf`, ignored for a `.blend`'s conversion.

use super::super::import::{import, import_from};

/// A material-only glTF whose one material is `doubleSided`, written to a temp file.
fn double_sided_gltf(name: &str) -> std::path::PathBuf {
    let dir = crate::test_temp::dir().join(format!("rusty_double_sided_{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("card.gltf");
    let json = r#"{"asset":{"version":"2.0"},"materials":[{"name":"Card","doubleSided":true}]}"#;
    std::fs::write(&path, json).unwrap();
    path
}

#[test]
fn an_authored_gltf_keeps_double_sided() {
    let asset = import(&double_sided_gltf("authored")).unwrap();
    assert!(asset.materials[0].double_sided);
}

#[test]
fn a_converted_blend_imports_single_sided() {
    let path = double_sided_gltf("blend");
    let asset = import_from(&path, Some("models/Card.blend")).unwrap();
    assert!(
        !asset.materials[0].double_sided,
        "Blender writes it by default"
    );
}
