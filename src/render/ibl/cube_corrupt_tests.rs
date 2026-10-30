//! Corrupt and unsupported cube files (#1609): every one is refused, by the header
//! read and the full parse alike, and none panics.

use super::cubemap_tests::{build_cubemap_ktx2, R8G8B8A8_SRGB};
use super::*;

/// A valid 4-texel cube with its three mips, laid out as the bake writes them.
fn mipped_cube() -> Vec<u8> {
    let mut bytes = build_cubemap_ktx2(4, R8G8B8A8_SRGB, 6);
    let index = HEADER + 3 * LEVEL_ENTRY;
    let base_at = u64_at(&bytes, HEADER) as usize;
    // Room for two more index entries, then the 2- and 1-texel levels appended.
    bytes.splice(index - 2 * LEVEL_ENTRY..index - 2 * LEVEL_ENTRY, [0; 48]);
    put_u32(&mut bytes, 40, 3);
    let dfd_at = u32_at(&bytes, 48);
    put_u32(&mut bytes, 48, dfd_at + 48);
    put_u64(&mut bytes, HEADER, base_at as u64 + 48);
    for (level, edge) in [(1, 2u64), (2, 1)] {
        let len = edge * edge * 24;
        let entry = HEADER + level * LEVEL_ENTRY;
        let at = bytes.len() as u64;
        put_u64(&mut bytes, entry, at);
        put_u64(&mut bytes, entry + 8, len);
        put_u64(&mut bytes, entry + 16, len);
        bytes.resize(bytes.len() + len as usize, 0);
    }
    bytes
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}
fn put_u32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn put_u64(b: &mut [u8], at: usize, v: u64) {
    b[at..at + 8].copy_from_slice(&v.to_le_bytes());
}

/// The same judgement from the header read (through a file) and the full parse.
fn judged(bytes: &[u8]) -> (Result<CubeShape, Refusal>, Option<CubeShape>) {
    let path = crate::test_temp::dir().join("cube.ktx2");
    std::fs::write(&path, bytes).unwrap();
    let header = read_cube_shape(&path.to_string_lossy());
    (
        header,
        parse_ktx2_cubemap_mips(bytes).ok().map(|c| c.shape()),
    )
}

#[test]
fn a_mipped_cube_is_read_whole_by_both() {
    let shape = CubeShape { size: 4, mips: 3 };
    assert_eq!(judged(&mipped_cube()), (Ok(shape), Some(shape)));
}

#[test]
fn every_corrupt_or_unsupported_cube_is_refused_by_both() {
    const LAST: usize = HEADER + 2 * LEVEL_ENTRY;
    let corrupt: [(&str, fn(&mut Vec<u8>)); 9] = [
        ("truncated", |b| b.truncate(b.len() - 1)),
        ("truncated in the header", |b| b.truncate(60)),
        ("truncated in the index", |b| b.truncate(HEADER + 30)),
        ("levelCount past log2(size)+1", |b| put_u32(b, 40, 4)),
        ("levelCount huge", |b| put_u32(b, 40, u32::MAX)),
        ("zstd-supercompressed", |b| put_u32(b, 44, 2)),
        ("a 3D cube", |b| put_u32(b, 28, 4)),
        ("short last level", |b| put_u64(b, LAST + 8, 23)),
        ("a level index past the end", |b| {
            put_u64(b, LAST, u64::MAX - 8)
        }),
    ];
    for (what, corrupt) in corrupt {
        let mut bytes = mipped_cube();
        corrupt(&mut bytes);
        let (header, parse) = judged(&bytes);
        assert!(header.is_err(), "{what}: the header read refuses it");
        assert_eq!(parse, None, "{what}: the parse refuses it");
    }
}

/// Every prefix of a cube and every single corrupted byte is refused or read, never
/// a panic: the parse runs every one.
#[test]
fn no_prefix_or_flipped_byte_panics() {
    let bytes = mipped_cube();
    for end in 0..bytes.len() {
        assert!(parse_ktx2_cubemap_mips(&bytes[..end]).is_err());
    }
    for at in 0..HEADER + 3 * LEVEL_ENTRY {
        let mut flipped = bytes.clone();
        flipped[at] ^= 0xFF;
        let _ = parse_ktx2_cubemap_mips(&flipped);
        let _ = cube_shape(flipped[..48].try_into().unwrap());
    }
}
