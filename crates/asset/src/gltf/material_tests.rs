//! A glTF material's `doubleSided` on import (honoured for an authored `.gltf`, ignored
//! for a `.blend`'s conversion), its `KHR_materials_ior` and `KHR_materials_specular`
//! factors, and its occlusion and specular-level textures.

use super::super::import::{import, import_from};

/// A material-only glTF whose one material is `doubleSided`, written to a temp file.
fn double_sided_gltf(name: &str) -> std::path::PathBuf {
    let dir = rusty_test_support::temp::dir().join(format!("rusty_double_sided_{name}"));
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
    let dir = rusty_test_support::temp::dir().join(format!("rusty_gltf_material_{name}"));
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

#[test]
fn an_orm_gltf_names_one_image_for_occlusion_and_metallic_roughness() {
    // Texture 0 is the ORM image (occlusion and metallic-roughness), 1 the
    // `KHR_materials_specular` level.
    let ext = r#"{"name":"Felt","occlusionTexture":{"index":0,"strength":0.25},
        "pbrMetallicRoughness":{"metallicRoughnessTexture":{"index":0}},
        "extensions":{"KHR_materials_specular":{"specularTexture":{"index":1}}}}],
        "textures":[{"source":0},{"source":1}],
        "images":[{"uri":"orm.png"},{"uri":"level.png"}"#;
    let m = &import(&material_gltf("orm", ext)).unwrap().materials[0];
    assert_eq!(
        m.occlusion_map, m.metallic_roughness_map,
        "one image, one path"
    );
    let named = |map: &Option<crate::MapRef>, file: &str| {
        map.as_ref().is_some_and(|p| p.as_str().ends_with(file))
    };
    assert!(named(&m.occlusion_map, "orm.png"), "{:?}", m.occlusion_map);
    assert!(named(&m.specular_map, "level.png"), "{:?}", m.specular_map);
    assert_eq!(m.occlusion_strength, 0.25);
}

#[test]
fn a_gltf_without_occlusion_keeps_its_strength_whole() {
    let m = &import(&material_gltf("no_ao", r#"{"name":"Plain"}"#))
        .unwrap()
        .materials[0];
    assert_eq!((&m.occlusion_map, m.occlusion_strength), (&None, 1.0));
    assert_eq!(m.specular_map, None);
}

#[test]
fn a_map_off_uv0_is_named_for_its_warning() {
    let json = r#"{"asset":{"version":"2.0"},"materials":[{"name":"Felt",
        "occlusionTexture":{"index":0,"texCoord":1},
        "pbrMetallicRoughness":{"baseColorTexture":{"index":0}}}],
        "textures":[{"source":0}],"images":[{"uri":"ao.png"}]}"#;
    let gltf = gltf::Gltf::from_slice(json.as_bytes()).unwrap();
    let material = gltf.materials().next().unwrap();
    assert_eq!(super::off_uv0(&material), [("occlusion map", 1)]);
}
