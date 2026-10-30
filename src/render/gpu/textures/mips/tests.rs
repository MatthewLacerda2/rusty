//! The mip chain's settings (#1322): how many levels an upload gets.

use super::*;

fn settings(kind: TextureKind) -> MipSettings {
    MipSettings::for_map(&TextureSettings::default(), kind)
}

#[test]
fn a_full_chain_runs_down_to_one_texel() {
    assert_eq!(full_chain_len(1, 1), 1);
    assert_eq!(full_chain_len(2, 2), 2);
    assert_eq!(full_chain_len(2048, 2048), 12);
    assert_eq!(full_chain_len(300, 17), 9);
}

#[test]
fn a_material_texture_gets_a_full_chain_by_default() {
    for kind in [TextureKind::Colour, TextureKind::Data] {
        assert!(settings(kind).generate, "Unity's default is a full chain");
        assert_eq!(settings(kind).levels(1024, 512), 11);
    }
}

#[test]
fn with_generation_off_only_the_base_is_uploaded() {
    let import = TextureSettings {
        generate_mipmaps: false,
        ..TextureSettings::default()
    };
    let off = MipSettings::for_map(&import, TextureKind::Colour);
    assert_eq!(off.levels(64, 64), 1);
}

#[test]
fn each_kind_averages_through_its_own_view() {
    assert_eq!(
        view_format(TextureKind::Colour),
        wgpu::TextureFormat::Rgba8UnormSrgb
    );
    assert_eq!(
        view_format(TextureKind::Data),
        wgpu::TextureFormat::Rgba8Unorm
    );
}
