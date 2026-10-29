//! A material map uploads block-compressed (#1574): the format follows its slot,
//! every mip comes from the import cache, and it draws what its uncompressed
//! twin draws. On a GPU without BC every one falls back to RGBA8.

use std::path::Path;

use image::{Rgba, RgbaImage};

use super::compressed::{gpu_bytes, FEATURE};
use super::freshness_tests::{centre, emissive_scene, temp_png};
use super::TextureKind;
use crate::asset::sidecar::{set_texture, Compression, TextureSettings};
use crate::asset::MapRef;
use crate::components::MaterialAsset;
use crate::render::Renderer;
use wgpu::TextureFormat as F;

/// A 16² PNG of `texel` everywhere at a fresh path named `name`.
fn flat(name: &str, texel: [u8; 4]) -> String {
    let path = temp_png(name);
    RgbaImage::from_pixel(16, 16, Rgba(texel))
        .save(&path)
        .unwrap();
    path
}

fn format(renderer: &Renderer, path: &str, kind: TextureKind) -> F {
    let texture = renderer.upload_map(&MapRef::from(path), kind).unwrap();
    assert_eq!(texture.texture.mip_level_count(), 5, "{path}: every mip");
    texture.texture.format()
}

#[test]
fn gpu_each_slot_uploads_in_its_block_format_with_every_mip() {
    let Some(renderer) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let colour = flat("bc_colour", [200, 100, 50, 255]);
    let clear = flat("bc_clear", [200, 100, 50, 128]);
    let grey = flat("bc_grey", [90, 90, 90, 255]);
    let bc = renderer.device.features().contains(FEATURE);
    let cases = [
        (&colour, TextureKind::Colour, F::Bc1RgbaUnormSrgb),
        (&clear, TextureKind::Colour, F::Bc7RgbaUnormSrgb),
        (&colour, TextureKind::Normal, F::Bc5RgUnorm),
        (&grey, TextureKind::Mask, F::Bc4RUnorm),
        (&colour, TextureKind::Mask, F::Bc1RgbaUnormSrgb),
    ];
    for (path, kind, want) in cases {
        let want = if bc { want } else { F::Rgba8UnormSrgb };
        assert_eq!(format(&renderer, path, kind), want, "{kind:?}");
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
    let packed = flat("bc_drawn", texel);
    let raw = flat("bc_drawn_raw", texel);
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

#[test]
fn gpu_a_one_channel_roughness_map_reads_its_red_channel() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let grey = flat("bc_roughness", [180, 180, 180, 255]);
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
}
