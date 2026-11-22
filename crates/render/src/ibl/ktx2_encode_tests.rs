use super::*;
use crate::ibl::cubemap::parse_ktx2_cubemap_mips;
use crate::ibl::reflection_prefilter::source::{decode_face, encode_face};
use crate::ibl::reflection_prefilter::{PrefilteredCubemap, TEXEL_BYTES};

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

fn face(bytes: &[u8], level: usize, f: usize) -> Vec<glam::Vec3> {
    let cube = parse_ktx2_cubemap_mips(bytes).unwrap().decoded();
    let size = (cube.size >> level).max(1) as usize;
    let face_bytes = size * size * TEXEL_BYTES;
    decode_face(&cube.mips[level][f * face_bytes..][..face_bytes])
}

#[test]
fn encode_round_trips_through_loader() {
    let cube = tiny_cube(4);
    let bytes = encode_cubemap(&cube);
    let parsed = parse_ktx2_cubemap_mips(&bytes).expect("encoded KTX2 should parse");
    assert_eq!(
        (parsed.size, parsed.format),
        (4, wgpu::TextureFormat::Bc6hRgbUfloat)
    );
    assert_eq!(parsed.mips[0].len(), 16 * 6, "one 16-byte block a face");
    for f in 0..6 {
        let red = face(&bytes, 0, f)[0].x;
        assert!(
            (red - f as f32 * 10.0).abs() <= 0.05 * f as f32 * 10.0,
            "{f}: {red}"
        );
    }
}

#[test]
fn encode_round_trips_full_mip_chain() {
    let cube = tiny_cube(8);
    let bytes = encode_cubemap(&cube);
    let mips = parse_ktx2_cubemap_mips(&bytes).expect("encoded KTX2 should parse mips");
    assert_eq!(mips.size, 8);
    assert_eq!(mips.mips.len(), cube.level_count());
    let lens: Vec<usize> = mips.mips.iter().map(Vec::len).collect();
    assert_eq!(lens, [4 * 16 * 6, 16 * 6, 16 * 6, 16 * 6]);
    for level in 0..4 {
        let size = (8usize >> level).max(1);
        let texels = face(&bytes, level, 3);
        assert_eq!(texels.len(), size * size);
        assert!(texels.iter().all(|t| (t.x - 30.0).abs() < 1.5 && t.y < 0.1));
    }
}

#[test]
fn a_baked_cube_round_trips_within_bc6h_error_and_deterministically() {
    let size = 16u32;
    let texel = |f: usize, i: u32| {
        let (x, y) = ((i % size) as f32, (i / size) as f32);
        let tint = [
            glam::Vec3::new(1.0, 0.6, 0.3),
            glam::Vec3::new(0.2, 0.5, 1.0),
        ][f % 2];
        let lamp = if f == 2 { 300.0 } else { 1.0 };
        tint * (0.5 + 2.0 * x / size as f32 + y / size as f32) * lamp
    };
    let faces = |f: usize| (0..size * size).map(|i| texel(f, i)).collect::<Vec<_>>();
    let cube = PrefilteredCubemap {
        mips: vec![std::array::from_fn(|f| encode_face(&faces(f)))],
        base_size: size,
    };
    let bytes = encode_cubemap(&cube);
    assert_eq!(
        bytes,
        encode_cubemap(&cube),
        "the same cube, the same bytes"
    );
    let block_max = |want: &[glam::Vec3], i: u32| {
        let (bx, by) = (i % size / 4 * 4, i / size / 4 * 4);
        let at = |k: u32| want[((by + k / 4) * size + bx + k % 4) as usize];
        (0..16).map(|k| at(k).max_element()).fold(0.0, f32::max)
    };
    for f in 0..6 {
        let want = faces(f);
        for (i, got) in face(&bytes, 0, f).iter().enumerate() {
            let err = (*got - want[i]).abs().max_element();
            // BC6H's error is relative to its block: 5% of its brightest channel.
            let limit = 0.05 * block_max(&want, i as u32);
            assert!(err <= limit, "{f}: {got} vs {}", want[i]);
        }
    }
}

#[test]
fn ktx2_reader_accepts_encoded_bytes() {
    let cube = tiny_cube(4);
    let bytes = encode_cubemap(&cube);
    let reader = ktx2::Reader::new(&bytes).expect("ktx2 crate should accept our encode");
    let header = reader.header();
    assert_eq!(header.face_count, 6);
    assert_eq!(header.pixel_width, 4);
    assert_eq!(header.level_count, cube.level_count() as u32);
    assert_eq!(header.format, Some(ktx2::Format::BC6H_UFLOAT_BLOCK));
}
