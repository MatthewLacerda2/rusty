//! A glTF material's `doubleSided` on import (#1615): honoured for an authored
//! `.gltf`, ignored for a `.blend`'s conversion; its `KHR_materials_ior` and
//! `KHR_materials_specular` factors (#1998).

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

/// A material-only glTF of one material, its `json` written to a temp file.
fn material_gltf(name: &str, material: &str) -> std::path::PathBuf {
    let dir = crate::test_temp::dir().join(format!("rusty_gltf_material_{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("m.gltf");
    let json = format!(r#"{{"asset":{{"version":"2.0"}},"materials":[{material}]}}"#);
    std::fs::write(&path, json).unwrap();
    path
}

#[test]
fn a_gltf_with_ior_and_specular_imports_both() {
    let ext = r#"{"name":"Water","extensions":{"KHR_materials_ior":{"ior":1.33},
        "KHR_materials_specular":{"specularFactor":0.5,"specularColorFactor":[2,1,0.5]}}}"#;
    let m = &import(&material_gltf("ext", ext)).unwrap().materials[0];
    assert_eq!(
        (m.ior, m.specular, m.specular_color),
        (1.33, 0.5, [2.0, 1.0, 0.5])
    );
}

#[test]
fn a_gltf_without_them_keeps_the_extensions_defaults() {
    let m = &import(&material_gltf("plain", r#"{"name":"Plain"}"#))
        .unwrap()
        .materials[0];
    assert_eq!((m.ior, m.specular, m.specular_color), (1.5, 1.0, [1.0; 3]));
}
