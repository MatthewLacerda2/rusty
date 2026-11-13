//! A lightmap page round trip (#1694): BC6H holds linear HDR within 5%
//! with no RGBM range cap, the KTX2 file reads back exactly what was written in
//! the spec's level layout, and a malformed file is refused.

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

/// A BC6H page nobody stops.
fn bc6h(edge: u32, levels: &[Vec<Vec3>], threads: usize) -> Page {
    encode_bc6h(edge, levels, threads, &|| false).expect("never stopped")
}

#[test]
fn a_stopped_encode_makes_no_page() {
    let asked = std::cell::Cell::new(0);
    let stop_second = || {
        asked.set(asked.get() + 1);
        asked.get() > 1
    };
    assert_eq!(encode_bc6h(8, &[light(8), light(4)], 1, &stop_second), None);
    assert_eq!(asked.get(), 2, "asked before each level");
}

#[test]
fn bc6h_holds_hdr_light_within_a_few_percent() {
    let texels = light(64);
    let page = bc6h(64, std::slice::from_ref(&texels), 3);
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
    let back = decode_bc6h(&bc6h(4, &[texels], 1), 0);
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
    let one = bc6h(32, std::slice::from_ref(&texels), 1);
    assert_eq!(one, bc6h(32, &[texels], 5));
}

#[test]
fn a_ktx2_page_round_trips_in_the_specs_layout() {
    let page = bc6h(8, &[light(8), light(4)], 2);
    let file = write(&page);
    // The spec's layout: the index largest first, the images smallest first,
    // each 16-byte aligned.
    let offset = |l: usize| {
        let at = 80 + l * 24;
        u64::from_le_bytes(file[at..at + 8].try_into().unwrap())
    };
    assert!(offset(1) < offset(0), "the 4² level is stored first");
    assert!(offset(0) % 16 == 0 && offset(1) % 16 == 0, "block aligned");
    assert_eq!(read(&file), Ok(page));
}

#[test]
fn a_malformed_file_is_refused_naming_why() {
    let page = bc6h(8, &[light(8), light(4)], 1);
    let file = write(&page);
    assert_eq!(read(&file).as_ref(), Ok(&page));
    assert!(read(&file[..file.len() - 1]).is_err(), "truncated");
    assert!(read(&file[..40]).is_err(), "header only");
    assert!(read(b"not a ktx2").is_err());
    let mut other = file.clone();
    other[12..16].copy_from_slice(&145u32.to_le_bytes()); // BC7_UNORM
    assert!(read(&other).unwrap_err().contains("format"));
    let mut wide = file;
    wide[20..24].copy_from_slice(&16u32.to_le_bytes());
    assert!(read(&wide).is_err(), "not square, levels the wrong length");
}

/// A direction page (#1996): RGBA8 levels round trip as they are, and a level of
/// the wrong length is refused.
#[test]
fn an_rgba8_page_round_trips_and_its_lengths_are_checked() {
    let level = |edge: u32| (0..edge * edge * 4).map(|i| (i * 7) as u8).collect();
    let page = Page {
        format: PageFormat::Rgba8,
        edge: 8,
        levels: vec![level(8), level(4)],
    };
    assert_eq!(page.gpu_bytes(), 8 * 8 * 4 + 4 * 4 * 4);
    let file = write(&page);
    assert_eq!(read(&file), Ok(page.clone()));
    let short = Page {
        levels: vec![level(8), vec![0; 15]],
        ..page
    };
    assert!(read(&write(&short)).unwrap_err().contains("level 1"));
}

#[test]
fn levels_stop_at_whole_blocks() {
    assert_eq!(max_levels(2048), 10);
    assert_eq!(max_levels(32), 4);
    assert_eq!(max_levels(4), 1);
    assert_eq!(max_levels(6), 0);
}

/// A level under a block (a cube's 2×2 and 1×1 mips, #1885) is one padded block,
/// and decodes to its own texels.
#[test]
fn a_level_under_a_block_is_one_padded_block() {
    for edge in [1u32, 2] {
        let texels: Vec<Vec3> = (0..edge * edge)
            .map(|i| Vec3::new(1.0 + i as f32, 0.5, 4.0))
            .collect();
        let blocks = super::bc6h_level(edge, &texels, 3);
        assert_eq!(blocks.len(), 16);
        let back = super::bc6h_texels(edge, &blocks);
        assert_eq!(back.len(), texels.len());
        for (got, want) in back.iter().zip(&texels) {
            assert!(
                (*got - *want).abs().max_element() < 0.05 * want.max_element(),
                "{got} {want}"
            );
        }
    }
}
