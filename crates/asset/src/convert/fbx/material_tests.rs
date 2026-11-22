use super::super::{convert_and_import, fixture};
use crate::{MapRef, MaterialData};
use rusty_core::color::gamma_to_linear;

fn materials(name: &str) -> Vec<MaterialData> {
    let out = rusty_test_support::temp::dir().join(format!("{name}.glb"));
    convert_and_import(&fixture(name), &out).materials
}

fn material<'a>(materials: &'a [MaterialData], name: &str) -> &'a MaterialData {
    materials.iter().find(|m| m.name == name).expect(name)
}

#[test]
fn colours_bound_to_a_texture_are_white_times_their_factor() {
    let materials = materials("embedded_textures.fbx");
    let (red, blue) = (material(&materials, "Red"), material(&materials, "Blue"));
    assert_eq!(red.base_color, [1.0; 4]);
    // `Blue`'s `DiffuseColor` is black but bound to a texture: URP takes white × `DiffuseFactor`.
    let half = gamma_to_linear(0.5);
    assert_eq!(blue.base_color, [half, half, half, 1.0]);
    assert_eq!(red.emissive, [1.0; 3]);
    assert!(red.emissive_map.as_ref().and_then(MapRef::file).is_some());
}

#[test]
fn a_colour_whose_texture_is_not_written_stays_without_its_factor() {
    // The texture is missing: URP takes `DiffuseColor` as is, `DiffuseFactor` unapplied.
    let materials = materials("missing_texture.fbx");
    let gone = material(&materials, "Gone");
    assert_eq!(gone.base_color, [0.2, 0.4, 0.6, 1.0]);
    assert_eq!(gone.base_color_map, None);
    assert_eq!(gone.emissive, [gamma_to_linear(0.5), 0.0, 0.0]);
    assert_eq!(gone.emissive_map, None);
}

#[test]
fn a_physical_material_is_its_colour_times_its_weight() {
    // Not classic shading: the weight applies.
    let materials = materials("physical_material.fbx");
    let [r, g, b, _] = material(&materials, "Physical").base_color;
    assert_eq!([r, g, b], [0.4, 0.2, 0.8]);
}
