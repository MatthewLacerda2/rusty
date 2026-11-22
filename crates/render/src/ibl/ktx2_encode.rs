//! Hand-rolled, as the `ktx2` crate only parses; the bytes must round-trip through
//! `cubemap::parse_ktx2_cubemap_mips`. A mip under 4×4 is one padded block per face.

use glam::Vec3;

use crate::ibl::reflection_prefilter::{PrefilteredCubemap, TEXEL_BYTES};
use crate::CubemapFace;
use rusty_asset::lightmap_page::bc6h_level;
use rusty_core::color::f16_to_f32;

const MAGIC: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];
/// Bytes: the KTX2 header and one level-index entry, as `cubemap`'s parse reads them.
const HEADER_LEN: usize = 80;
const LEVEL_INDEX_ENTRY_LEN: usize = 24;
pub(crate) const VK_FORMAT_BC6H_UFLOAT_BLOCK: u32 = 143;

pub fn encode_cubemap(cube: &PrefilteredCubemap) -> Vec<u8> {
    debug_assert!(
        crate::ibl::reflection_prefilter::validate_layout(cube),
        "prefiltered cube has a malformed mip layout"
    );
    let levels = cube.level_count();
    let dfd = build_dfd();
    let dfd_offset = HEADER_LEN + levels * LEVEL_INDEX_ENTRY_LEN;
    let level_data_offset = dfd_offset + dfd.len();

    let mut level_bytes: Vec<u8> = Vec::new();
    let mut index: Vec<(u64, u64)> = Vec::with_capacity(levels);
    for level in 0..levels {
        let offset = level_data_offset as u64 + level_bytes.len() as u64;
        let start = level_bytes.len();
        let edge = cube.level_size(level);
        for face in CubemapFace::ALL {
            let texels = half_texels(&cube.mips[level][face as usize]);
            level_bytes.extend_from_slice(&bc6h_level(edge, &texels, threads()));
        }
        index.push((offset, (level_bytes.len() - start) as u64));
    }

    let mut buf = vec![0u8; level_data_offset];
    write_header(
        &mut buf,
        cube.base_size,
        levels as u32,
        dfd_offset,
        dfd.len(),
    );
    write_level_index(&mut buf, &index);
    buf[dfd_offset..level_data_offset].copy_from_slice(&dfd);
    buf.extend_from_slice(&level_bytes);
    buf
}

fn half_texels(bytes: &[u8]) -> Vec<Vec3> {
    let half = |b: &[u8]| f16_to_f32(u16::from_le_bytes([b[0], b[1]]));
    (bytes.chunks_exact(TEXEL_BYTES))
        .map(|t| Vec3::new(half(&t[..2]), half(&t[2..4]), half(&t[4..6])))
        .collect()
}

/// The blocks do not depend on the thread count, so a bake's cubes are the same on any machine.
fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, usize::from)
}

fn write_header(buf: &mut [u8], size: u32, levels: u32, dfd_offset: usize, dfd_len: usize) {
    buf[0..12].copy_from_slice(&MAGIC);
    put_u32(buf, 12, VK_FORMAT_BC6H_UFLOAT_BLOCK); // vkFormat
    put_u32(buf, 16, 1); // typeSize (1 for a block-compressed format)
    put_u32(buf, 20, size); // pixelWidth
    put_u32(buf, 24, size); // pixelHeight
    put_u32(buf, 28, 0); // pixelDepth (2D)
    put_u32(buf, 32, 0); // layerCount (not an array)
    put_u32(buf, 36, 6); // faceCount (cubemap)
    put_u32(buf, 40, levels); // levelCount
    put_u32(buf, 44, 0); // supercompressionScheme (none)
    put_u32(buf, 48, dfd_offset as u32); // dfdByteOffset
    put_u32(buf, 52, dfd_len as u32); // dfdByteLength
    put_u32(buf, 56, 0); // kvdByteOffset (none)
    put_u32(buf, 60, 0); // kvdByteLength
    put_u64(buf, 64, 0); // sgdByteOffset (none)
    put_u64(buf, 72, 0); // sgdByteLength
}

fn write_level_index(buf: &mut [u8], index: &[(u64, u64)]) {
    for (i, (offset, length)) in index.iter().enumerate() {
        let at = HEADER_LEN + i * LEVEL_INDEX_ENTRY_LEN;
        put_u64(buf, at, *offset);
        put_u64(buf, at + 8, *length);
        put_u64(buf, at + 16, *length);
    }
}

/// A minimal basic DFD: enough for `ktx2::Reader`'s checks; the loader never reads it.
fn build_dfd() -> Vec<u8> {
    let block_size: u32 = 24;
    let total: u32 = 4 + block_size;
    let mut dfd = Vec::with_capacity(total as usize);
    dfd.extend_from_slice(&total.to_le_bytes()); // dfdTotalSize
    dfd.extend_from_slice(&0u32.to_le_bytes());
    dfd.extend_from_slice(&((block_size << 16) | 2).to_le_bytes());
    dfd.extend_from_slice(&[0u8; 16]);
    dfd
}

fn put_u32(buf: &mut [u8], at: usize, v: u32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_u64(buf: &mut [u8], at: usize, v: u64) {
    buf[at..at + 8].copy_from_slice(&v.to_le_bytes());
}

#[cfg(test)]
#[path = "ktx2_encode_tests.rs"]
mod ktx2_encode_tests;
