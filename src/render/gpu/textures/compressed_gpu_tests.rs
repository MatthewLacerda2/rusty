//! A material map uploads block-compressed (#1574): the format follows its Texture
//! Type, every mip comes from the import cache, it draws what its uncompressed twin
//! draws, and a slot its type cannot serve reads an uncompressed copy. On a GPU
//! without BC every one falls back to RGBA8.

use std::path::Path;

use image::{Rgba, RgbaImage};

use super::compressed::{gpu_bytes, FEATURE};
use super::freshness_tests::{centre, emissive_scene, temp_png};
use super::TextureKind;
use crate::asset::sidecar::{set_texture, Compression, TextureSettings, TextureType};
use crate::asset::texture_import::refresh;
use crate::asset::MapRef;
use crate::components::MaterialAsset;
use crate::render::Renderer;
use wgpu::TextureFormat as F;

/// A 16² PNG of `texel` everywhere at a fresh path named `name`, of
/// `texture_type`, its compressed copy imported.
fn flat(name: &str, texel: [u8; 4], texture_type: TextureType) -> String {
    let path = temp_png(name);
    RgbaImage::from_pixel(16, 16, Rgba(texel))
        .save(&path)
        .unwrap();
    let settings = TextureSettings {
        texture_type,
        ..Default::default()
    };
    set_texture(Path::new(&path), settings).unwrap();
    refresh(Path::new(&path), "").unwrap();
    path
}

fn format(renderer: &Renderer, path: &str, kind: TextureKind) -> F {
    let texture = renderer.upload_map(&MapRef::from(path), kind).unwrap();
    assert_eq!(texture.texture.mip_level_count(), 5, "{path}: every mip");
    texture.texture.format()
}

#[test]
fn gpu_each_type_uploads_in_its_block_format_where_its_slot_reads_it() {
    let Some(renderer) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let colour = flat("bc_colour", [200, 100, 50, 128], TextureType::Default);
    let normal = flat("bc_normal", [128, 128, 255, 255], TextureType::NormalMap);
    let grey = flat("bc_grey", [90, 90, 90, 255], TextureType::SingleChannel);
    let bc = renderer.device.features().contains(FEATURE);
    let cases = [
        (&colour, TextureKind::Colour, F::Bc7RgbaUnormSrgb),
        (&colour, TextureKind::Mask, F::Bc7RgbaUnormSrgb),
        (&normal, TextureKind::Normal, F::Bc5RgUnorm),
        (&grey, TextureKind::Mask, F::Bc4RUnorm),
        // A slot the type cannot serve uploads uncompressed.
        (&normal, TextureKind::Colour, F::Rgba8UnormSrgb),
        (&grey, TextureKind::Data, F::Rgba8UnormSrgb),
    ];
    for (path, kind, want) in cases {
        let want = if bc { want } else { F::Rgba8UnormSrgb };
        assert_eq!(format(&renderer, path, kind), want, "{path} as {kind:?}");
    }
    let none = TextureSettings {
        compression: Compression::None,
        ..Default::default()
    };
    set_texture(Path::new(&colour), none).unwrap();
    let raw = renderer.upload_map(&MapRef::from(colour.as_str()), TextureKind::Colour);
    let raw = raw.unwrap();
    assert_eq!(raw.texture.format(), F::Rgba8UnormSrgb);
    // 16² + 8² + 4² + 2² + 1² texels at four bytes.
    assert_eq!(gpu_bytes(&raw.texture), 341 * 4);
    if !bc {
        eprintln!("no BC on this adapter: the fallback was tested");
    }
}

#[test]
fn gpu_a_compressed_map_draws_like_its_uncompressed_twin() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let texel = [40, 200, 120, 255];
    let packed = flat("bc_drawn", texel, TextureType::Default);
    let raw = flat("bc_drawn_raw", texel, TextureType::Default);
    let none = TextureSettings {
        compression: Compression::None,
        ..Default::default()
    };
    set_texture(Path::new(&raw), none).unwrap();
    let a = centre(&mut renderer, &emissive_scene(&packed));
    let b = centre(&mut renderer, &emissive_scene(&raw));
    let off = a.iter().zip(&b).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
    assert!(off <= 4, "compressed {a:?} vs uncompressed {b:?}");
    assert!(
        a[1] > 150 && a[0] < 100,
        "the map was drawn, not the default: {a:?}"
    );
}

/// A one-channel map first loaded as a mask (BC4), then drawn as emissive colour,
/// draws its grey, not BC4's red alone (#1574's review, finding 2).
#[test]
fn gpu_a_one_channel_map_reads_red_as_a_mask_and_grey_as_colour() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let grey = flat(
        "bc_roughness",
        [180, 180, 180, 255],
        TextureType::SingleChannel,
    );
    let map = MapRef::from(grey.as_str());
    renderer.load_texture_as(&map, TextureKind::Mask);
    let material = MaterialAsset {
        roughness_map: Some(map),
        ..Default::default()
    };
    let uniform = crate::render::draw::uniforms::material_uniform(true, Some(&material));
    let uniform = renderer.with_map_channels(uniform, Some(&material));
    let bc = renderer.device.features().contains(FEATURE);
    assert_eq!(uniform.use_roughness_map, if bc { 2 } else { 1 });
    assert_eq!(uniform.use_metallic_map, 0, "no metallic map");
    let drawn = centre(&mut renderer, &emissive_scene(&grey));
    let spread = drawn.iter().max().unwrap() - drawn.iter().min().unwrap();
    assert!(drawn[0] > 100 && spread <= 4, "grey, not red: {drawn:?}");
}
