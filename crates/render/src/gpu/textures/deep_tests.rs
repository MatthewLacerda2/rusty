use std::path::Path;

use image::{DynamicImage, ImageBuffer, Rgba};

use super::compressed::reads_as;
use super::deep::{stored, wanted, DEEP_FORMAT, SIGNED_NORMAL_MAP};
use super::freshness_tests::temp_png;
use super::TextureKind;
use crate::readback::read_texture_bytes;
use rusty_asset::sidecar::{set_texture, TextureSettings, TextureType};
use rusty_asset::MapRef;
use rusty_components::MaterialAsset;
use rusty_core::color::f16_to_f32;

// A 16-bit file's step in x, y or z once mapped to [-1, 1].
const FILE_STEP: f64 = 2.0 / 65535.0;

fn signed(v: u16) -> f64 {
    f64::from(v) / 65535.0 * 2.0 - 1.0
}

const KINDS: [TextureKind; 4] = [
    TextureKind::Colour,
    TextureKind::Data,
    TextureKind::Normal,
    TextureKind::Mask,
];

fn normal_map() -> TextureSettings {
    TextureSettings {
        texture_type: TextureType::NormalMap,
        ..Default::default()
    }
}

#[test]
fn only_a_normal_slot_of_a_16_bit_normal_map_uploads_at_16_bits() {
    let deep = DynamicImage::ImageRgba16(ImageBuffer::from_pixel(4, 4, Rgba([32768; 4])));
    let flat = DynamicImage::ImageRgba8(deep.to_rgba8());
    for kind in KINDS {
        let normal = kind == TextureKind::Normal;
        assert_eq!(wanted(&normal_map(), &deep, kind), normal, "{kind:?}");
        assert!(
            !wanted(&normal_map(), &flat, kind),
            "an 8-bit image, {kind:?}"
        );
        let default = TextureSettings::default();
        assert!(!wanted(&default, &deep, kind), "a Default image, {kind:?}");
    }
}

#[test]
fn a_16_bit_upload_serves_only_a_normal_slot() {
    for kind in KINDS {
        let normal = kind == TextureKind::Normal;
        assert_eq!(reads_as(DEEP_FORMAT, kind), normal, "{kind:?}");
        assert!(
            reads_as(wgpu::TextureFormat::Rgba8UnormSrgb, kind),
            "{kind:?}"
        );
    }
}

#[test]
fn the_shader_reads_the_flag_a_signed_upload_sets() {
    let wgsl = include_str!("../../../../../engine/shaders/forward/mapping.wgsl");
    let line = format!("const SIGNED_NORMAL_MAP: u32 = {SIGNED_NORMAL_MAP}u;");
    assert!(wgsl.contains(&line), "{line}");
}

#[test]
fn near_flat_every_step_of_the_file_survives_the_half_float() {
    let near_flat = (0..=u16::MAX).filter(|&v| signed(v).abs() < 1.0 / 16.0);
    for v in near_flat {
        let kept = f64::from(f16_to_f32(stored(0, v)));
        let error = (kept - signed(v)).abs();
        assert!(error <= FILE_STEP / 2.0, "{v}: {kept} for {}", signed(v));
    }
    assert_eq!(f16_to_f32(stored(1, 0)), -1.0);
    assert_eq!(f16_to_f32(stored(2, u16::MAX)), 1.0);
    assert_eq!(f16_to_f32(stored(3, u16::MAX)), 1.0, "alpha stays 0..1");
    assert_eq!(f16_to_f32(stored(3, 0)), 0.0);
}

// A polish's relief: slopes within ±0.0012 a texel apart, so a handful of the
// file's steps, each kept on the GPU in the material's flag and its texels.
#[test]
fn gpu_a_polish_keeps_its_16_bit_slopes_on_the_gpu() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let slope = |i: u32| (32768.0 + 39.3 * (f64::from(i) - 8.0) / 8.0).round() as u16;
    let image = ImageBuffer::from_fn(16, 16, |x, y| Rgba([slope(x), slope(y), 65535, 65535]));
    let path = temp_png("polish_slopes");
    image.save(&path).unwrap();
    set_texture(Path::new(&path), normal_map()).unwrap();
    let map = MapRef::from(path.as_str());
    let texture = renderer.load_texture_as(&map, TextureKind::Normal);
    assert_eq!(texture.texture.format(), DEEP_FORMAT);
    let (device, queue) = (&renderer.device, &renderer.queue);
    let bytes = read_texture_bytes(device, queue, &texture.texture, (16, 16), 8);
    let half = |b: &[u8]| f64::from(f16_to_f32(u16::from_le_bytes([b[0], b[1]])));
    for (texel, file) in bytes.chunks(8).zip(image.pixels()) {
        for c in 0..2 {
            let error = (half(&texel[2 * c..]) - signed(file[c])).abs();
            assert!(error <= FILE_STEP / 2.0, "{:?}: off by {error}", file.0);
        }
    }
    let material = MaterialAsset {
        normal_map: Some(map),
        ..Default::default()
    };
    let record = crate::draw::uniforms::material_data(true, Some(&material));
    let record = renderer.with_map_channels(record, Some(&material));
    assert_eq!(record.use_normal_map, SIGNED_NORMAL_MAP);
}
