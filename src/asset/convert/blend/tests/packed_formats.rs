//! Packed images that are not PNG or JPEG (#1072): `packed_formats.blend` packs a
//! blue BMP (`Bitmap`) and a float OpenEXR of linear 0.5 grey (`Radiance`). Each
//! imports as a decodable 8-bit PNG. Needs Blender 5+, so skipped by default (see
//! `tests`).

use super::{base_color_map, fixture, pixel};
use crate::asset::convert::{converter_for, refresh, textures_dirs};
use crate::asset::gltf::import::import_from;

#[test]
// lint: allow(skip) needs Blender 5+; CI's `blender` job installs it and runs this
#[ignore]
fn a_packed_bmp_and_a_packed_float_image_import_as_pngs() {
    let source = fixture("packed_formats.blend");
    let out = crate::test_temp::dir().join("packed_formats.blend.glb");
    refresh(converter_for(&source).unwrap(), &source, &out).expect("converts");
    let asset = import_from(&out, Some(&source.to_string_lossy())).expect("imports");
    let named = |identity: &str| format!("{}#tex/{identity}", source.display());
    assert_eq!(base_color_map(&asset, "Bmp"), named("Bitmap"));
    assert_eq!(base_color_map(&asset, "Float"), named("Radiance"));
    let [dir] = textures_dirs(&out).try_into().expect("one textures folder");
    for name in ["Bitmap.png", "Radiance.png"] {
        let bytes = std::fs::read(dir.join(name)).expect(name);
        assert!(bytes.starts_with(b"\x89PNG"), "{name} is a PNG");
    }
    assert_eq!(pixel(&out, "Bmp"), [0, 0, 255, 255], "blue");
    // Linear 0.5, encoded sRGB in 8 bits.
    let [r, g, b, a] = pixel(&out, "Float");
    assert!(
        [r, g, b].iter().all(|c| c.abs_diff(188) <= 1),
        "{r} {g} {b}"
    );
    assert_eq!(a, 255);
}
