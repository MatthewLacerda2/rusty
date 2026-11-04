//! Tests for the hand-rolled KTX2 encoder (#245). Sibling of `ktx2_encode.rs`; shares its
//! 300-line cap. Pure bytes in/out — no GPU. The key contract: what we ENCODE the #244
//! loader can PARSE back (the bake -> write -> load round-trip the issue requires).

use super::*;
use crate::render::ibl::cubemap::parse_ktx2_cubemap_mips;
use crate::render::ibl::reflection_prefilter::source::{decode_face, encode_face};
use crate::render::ibl::reflection_prefilter::{PrefilteredCubemap, TEXEL_BYTES};

/// A tiny prefiltered cube: `base`-edge mip0 down to 1×1, each level six solid faces tinted
/// `face_index * 10` in red (HDR past 1) so face order is checkable after the round-trip.
fn tiny_cube(base: u32) -> PrefilteredCubemap {
    let levels = 32 - base.leading_zeros();
    let mut mips = Vec::new();
    for level in 0..levels {
        let size = (base >> level).max(1);
        let faces: [Vec<u8>; 6] = std::array::from_fn(|f| {
            let red = glam::Vec3::X * (f as f32 * 10.0);
            encode_face(&vec![red; (size * size) as usize])
        });
        mips.push(faces);
    }
    PrefilteredCubemap {
        mips,
        base_size: base,
    }
}

/// The encoded bytes parse with the production loader, recovering size + face order.
#[test]
fn encode_round_trips_through_loader() {
    let cube = tiny_cube(4);
    let bytes = encode_cubemap(&cube);

    let parsed = parse_ktx2_cubemap_mips(&bytes).expect("encoded KTX2 should parse");
    assert_eq!(parsed.size, 4);
    let face_bytes = 4 * 4 * TEXEL_BYTES;
    assert_eq!(parsed.mips[0].len(), face_bytes * 6);
    // Face f's first texel's red is f*10 (the tint), proving face order survived.
    for f in 0..6 {
        let texel = decode_face(&parsed.mips[0][f * face_bytes..][..TEXEL_BYTES])[0];
        assert_eq!(texel.x, f as f32 * 10.0);
    }
}

/// The full mip chain parses back with the same level count and halving sizes.
#[test]
fn encode_round_trips_full_mip_chain() {
    let cube = tiny_cube(4);
    let bytes = encode_cubemap(&cube);

    let mips = parse_ktx2_cubemap_mips(&bytes).expect("encoded KTX2 should parse mips");
    assert_eq!(mips.size, 4);
    assert_eq!(mips.mips.len(), cube.level_count());
    // Level 0 holds 4×4×6 RGBA16F, level 1 holds 2×2×6, level 2 holds 1×1×6.
    assert_eq!(mips.mips[0].len(), 4 * 4 * TEXEL_BYTES * 6);
    assert_eq!(mips.mips[1].len(), 2 * 2 * TEXEL_BYTES * 6);
    assert_eq!(mips.mips[2].len(), TEXEL_BYTES * 6);
}

/// The raw `ktx2::Reader` (the crate we hand-roll against) accepts our bytes — i.e. the
/// header, level index and DFD we emit pass its integrity validation.
#[test]
fn ktx2_reader_accepts_encoded_bytes() {
    let cube = tiny_cube(2);
    let bytes = encode_cubemap(&cube);
    let reader = ktx2::Reader::new(&bytes).expect("ktx2 crate should accept our encode");
    let header = reader.header();
    assert_eq!(header.face_count, 6);
    assert_eq!(header.pixel_width, 2);
    assert_eq!(header.level_count, cube.level_count() as u32);
    assert_eq!(header.format, Some(ktx2::Format::R16G16B16A16_SFLOAT));
}
