//! The mip chain (#1322): its length, its sizes, and how each kind of texel averages.

use super::*;

fn settings(kind: TextureKind) -> MipSettings {
    MipSettings::for_map(&MapRef::from("assets/floor.png"), kind)
}

/// Black and white texels in a 2×2 checker: their one-texel mip is their average.
fn checker() -> image::RgbaImage {
    image::RgbaImage::from_fn(2, 2, |x, y| match (x + y) % 2 {
        0 => image::Rgba([0, 0, 0, 0]),
        _ => image::Rgba([255, 255, 255, 255]),
    })
}

#[test]
fn a_full_chain_runs_down_to_one_texel() {
    assert_eq!(full_chain_len(1, 1), 1);
    assert_eq!(full_chain_len(2048, 2048), 12);
    assert_eq!(full_chain_len(300, 17), 9);
    let sizes: Vec<_> = chain(image::RgbaImage::new(5, 2), settings(TextureKind::Data))
        .iter()
        .map(|level| level.dimensions())
        .collect();
    assert_eq!(sizes, [(5, 2), (2, 1), (1, 1)]);
}

#[test]
fn a_material_texture_gets_a_full_chain_by_default() {
    for kind in [TextureKind::Colour, TextureKind::Data] {
        assert!(settings(kind).generate, "Unity's default is a full chain");
    }
}

#[test]
fn with_generation_off_only_the_base_is_uploaded() {
    let off = MipSettings {
        generate: false,
        kind: TextureKind::Colour,
    };
    assert_eq!(chain(image::RgbaImage::new(64, 64), off).len(), 1);
}

#[test]
fn a_data_map_averages_its_raw_values() {
    let mip = &chain(checker(), settings(TextureKind::Data))[1];
    assert_eq!(mip.get_pixel(0, 0).0, [128, 128, 128, 128]);
}

#[test]
fn a_colour_map_averages_in_linear_light() {
    // Half the light of white is sRGB 188, not the byte midpoint 128; alpha stays
    // linear.
    let mip = &chain(checker(), settings(TextureKind::Colour))[1];
    assert_eq!(mip.get_pixel(0, 0).0, [188, 188, 188, 128]);
}

#[test]
fn a_flat_colour_survives_every_level() {
    let flat = image::RgbaImage::from_pixel(8, 4, image::Rgba([200, 40, 10, 255]));
    for kind in [TextureKind::Colour, TextureKind::Data] {
        for level in chain(flat.clone(), settings(kind)) {
            assert!(
                level.pixels().all(|p| p.0 == [200, 40, 10, 255]),
                "{kind:?}"
            );
        }
    }
}
