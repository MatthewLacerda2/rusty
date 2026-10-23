//! FBX materials' base colours through the whole conversion.

use super::super::convert_to;
use crate::asset::{gltf, MaterialData};
use std::path::PathBuf;

/// `embedded_textures.fbx` converted fresh, then imported: its materials.
fn materials() -> Vec<MaterialData> {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/models/embedded_textures.fbx");
    let out = crate::test_temp::dir().join("quad.fbx.glb");
    convert_to(&source, &out).expect("the fixture converts");
    gltf::import::import(&out)
        .expect("its glTF imports")
        .materials
}

fn base_color(materials: &[MaterialData], name: &str) -> [f32; 4] {
    materials
        .iter()
        .find(|m| m.name == name)
        .expect(name)
        .base_color
}

#[test]
fn a_material_with_only_a_texture_has_a_white_base_colour() {
    // `Red` names no `DiffuseColor`: the texture alone colours it, so the factor
    // it multiplies is glTF's white, not black (#1280).
    assert_eq!(base_color(&materials(), "Red"), [1.0; 4]);
}

#[test]
fn a_colour_bound_to_a_texture_is_white_times_its_factor() {
    // `Blue` sets `DiffuseColor` to black *and* binds its texture there, as
    // exporters do: Unity reads white × `DiffuseFactor` (0.5), never the black
    // that would hide the texture (#1287).
    assert_eq!(base_color(&materials(), "Blue"), [0.5, 0.5, 0.5, 1.0]);
}
