//! FBX materials' colours through the whole conversion, as URP's FBX import
//! reads them (#1287, #1293).

use super::super::{convert_and_import, fixture};
use crate::asset::{MapRef, MaterialData};
use crate::core::color::gamma_to_linear;

/// The fixture `name` converted fresh, then imported: its materials.
fn materials(name: &str) -> Vec<MaterialData> {
    let out = crate::test_temp::dir().join(format!("{name}.glb"));
    convert_and_import(&fixture(name), &out).materials
}

fn material<'a>(materials: &'a [MaterialData], name: &str) -> &'a MaterialData {
    materials.iter().find(|m| m.name == name).expect(name)
}

#[test]
fn colours_bound_to_a_texture_are_white_times_their_factor() {
    let materials = materials("embedded_textures.fbx");
    let (red, blue) = (material(&materials, "Red"), material(&materials, "Blue"));
    // `Red` names no `DiffuseColor`: the texture alone colours it, so the factor
    // it multiplies is glTF's white, not black (#1280).
    assert_eq!(red.base_color, [1.0; 4]);
    // `Blue` sets `DiffuseColor` to black *and* binds its texture there, as
    // exporters do: URP takes white × `DiffuseFactor` (0.5), never the black that
    // would hide the texture (#1287), and its shader sees that colour linearised.
    let half = gamma_to_linear(0.5);
    assert_eq!(blue.base_color, [half, half, half, 1.0]);
    // `Red`'s emissive colour is a grey placeholder under its map: white, and the
    // map shows as itself (#1293).
    assert_eq!(red.emissive, [1.0; 3]);
    assert!(red.emissive_map.as_ref().and_then(MapRef::file).is_some());
    // `Blue`'s is a plain colour × `EmissiveFactor`, linearised.
    assert_eq!(blue.emissive, [half, 0.0, 0.0]);
    assert_eq!(blue.emissive_map, None);
}

#[test]
fn a_colour_whose_texture_is_not_written_stays_without_its_factor() {
    // `Gone`'s texture is nowhere: `DiffuseColor` is the best guess, and URP takes
    // it as is, `DiffuseFactor` (0.5) unapplied (#1293).
    let materials = materials("missing_texture.fbx");
    let gone = material(&materials, "Gone");
    assert_eq!(gone.base_color, [0.2, 0.4, 0.6, 1.0]);
    assert_eq!(gone.base_color_map, None);
}
