use image::{DynamicImage, ImageBuffer, Rgba};

use super::compressed::reads_as;
use super::deep::{wanted, DEEP_FORMAT};
use super::TextureKind;
use rusty_asset::sidecar::{TextureSettings, TextureType};

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
fn a_16_bit_upload_serves_every_slot_but_colour() {
    for kind in KINDS {
        assert_eq!(reads_as(DEEP_FORMAT, kind), !kind.is_colour(), "{kind:?}");
        assert!(
            reads_as(wgpu::TextureFormat::Rgba8UnormSrgb, kind),
            "{kind:?}"
        );
    }
}
