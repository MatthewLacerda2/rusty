//! A BC5 normal map keeps what its mips' normals lost (#1702): the import cache's
//! table rides the upload into the material record, which the shader reads in
//! place of the length BC5 cannot store.

use image::{Rgba, RgbaImage};

use super::compressed::FEATURE;
use super::compressed_gpu_tests::typed;
use super::TextureKind;
use rusty_asset::sidecar::TextureType;
use rusty_asset::MapRef;
use rusty_components::MaterialAsset;
use wgpu::TextureFormat as F;

/// A busy `NormalMap` uploads BC5 with what its mips' normals lost, and its
/// record reads that table (`use_normal_map` 2), as a map set through
/// `Material.SetNormalMap` or a glTF import does (#1702's review, item 1).
#[test]
fn gpu_a_bc5_normal_map_carries_its_mips_spread_into_the_record() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let tilt = |x: u32, _| Rgba([[64, 192][x as usize % 2], 128, 238, 255]);
    let bumps = typed(
        "bc_bumps",
        RgbaImage::from_fn(16, 16, tilt),
        TextureType::NormalMap,
    );
    let map = MapRef::from(bumps.as_str());
    let texture = renderer.load_texture_as(&map, TextureKind::Normal);
    let material = MaterialAsset {
        normal_map: Some(map),
        ..Default::default()
    };
    let uniform = crate::draw::uniforms::material_data(true, Some(&material));
    let uniform = renderer.with_map_channels(uniform, Some(&material));
    if !renderer.device.features().contains(FEATURE) {
        assert_eq!((texture.normal_variance, uniform.use_normal_map), (None, 1));
        return eprintln!("no BC on this adapter: the RGBA8 route was tested");
    }
    assert_eq!(texture.texture.format(), F::Bc5RgUnorm);
    let table = texture
        .normal_variance
        .expect("a BC5 upload carries its table");
    assert!(table[0] > 0, "{table:?}");
    assert_eq!(uniform.use_normal_map, 2);
    assert_eq!(uniform.normal_variance[0] & 0xff, u32::from(table[0]));
}
