//! A lightmap page round trip (#1694): BC6H holds linear HDR within 5%
//! with no RGBM range cap, BC7 a direction page within a few steps, the KTX2 file
//! reads back exactly what was written, and a malformed file is refused.

use glam::Vec3;

use super::*;

/// An `edge`² gradient of smooth HDR light, up past RGBM's old cap of 8.
fn light(edge: u32) -> Vec<Vec3> {
    (0..edge * edge)
        .map(|i| {
            let (x, y) = (
                (i % edge) as f32 / edge as f32,
                (i / edge) as f32 / edge as f32,
            );
            Vec3::new(9.0 + x * x * 3.0, 0.5 + y, 0.1 + x * y * 3.0)
        })
        .collect()
}

#[test]
fn bc6h_holds_hdr_light_within_a_few_percent() {
    let texels = light(64);
    let page = encode_bc6h(64, std::slice::from_ref(&texels), 3);
    assert_eq!(page.levels[0].len(), PageFormat::Bc6h.level_len(64));
    let back = decode_bc6h(&page, 0);
    for (a, b) in texels.iter().zip(&back) {
        let err = (*a - *b).abs().max_element();
        assert!(err <= a.max_element() * 0.05 + 1e-3, "{a} -> {b}");
    }
}

#[test]
fn bc6h_clamps_what_it_cannot_hold_and_black_stays_black() {
    let mut texels = vec![Vec3::ZERO; 16];
    texels[1] = Vec3::new(f32::NAN, -3.0, 1e9);
    let back = decode_bc6h(&encode_bc6h(4, &[texels], 1), 0);
    assert_eq!(back[0], Vec3::ZERO);
    assert!(
        back[1].x == 0.0 && back[1].y == 0.0 && back[1].z > 6.0e4,
        "{}",
        back[1]
    );
}

#[test]
fn the_same_page_encodes_the_same_on_any_thread_count() {
    let texels = light(32);
    let one = encode_bc6h(32, std::slice::from_ref(&texels), 1);
    assert_eq!(one, encode_bc6h(32, &[texels], 5));
}

#[test]
fn bc7_holds_a_direction_page_and_ktx2_round_trips() {
    let dirs: Vec<[u8; 4]> = (0..64u32)
        .map(|i| [(i * 4) as u8, 128, (255 - i * 4) as u8, (i * 3) as u8])
        .collect();
    let half: Vec<[u8; 4]> = dirs.iter().step_by(2).take(16).copied().collect();
    let page = encode_bc7(8, &[dirs.clone(), half], 2);
    for (a, b) in dirs.iter().zip(decode_bc7(&page, 0)) {
        let err = a.iter().zip(b).map(|(x, y)| x.abs_diff(y)).max().unwrap();
        assert!(err <= 12, "{a:?} -> {b:?}");
    }
    let file = write(&page);
    assert_eq!(read(&file), Ok(page));
}

#[test]
fn a_malformed_file_is_refused_naming_why() {
    let page = encode_bc6h(8, &[light(8), light(4)], 1);
    let file = write(&page);
    assert_eq!(read(&file).as_ref(), Ok(&page));
    assert!(read(&file[..file.len() - 1]).is_err(), "truncated");
    assert!(read(&file[..40]).is_err(), "header only");
    assert!(read(b"not a ktx2").is_err());
    let mut other = file.clone();
    other[12..16].copy_from_slice(&37u32.to_le_bytes()); // R8G8B8A8_UNORM
    assert!(read(&other).unwrap_err().contains("format"));
    let mut wide = file;
    wide[20..24].copy_from_slice(&16u32.to_le_bytes());
    assert!(read(&wide).is_err(), "not square, levels the wrong length");
}

#[test]
fn levels_stop_at_whole_blocks() {
    assert_eq!(max_levels(2048), 10);
    assert_eq!(max_levels(32), 4);
    assert_eq!(max_levels(4), 1);
    assert_eq!(max_levels(6), 0);
}
