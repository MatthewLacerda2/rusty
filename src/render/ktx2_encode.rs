//! src/render/ktx2_encode.rs — hand-rolled KTX2 writer for baked cubemaps (#245).
//!
//! The `ktx2` crate (0.3.0) is PARSE-ONLY — it has a `Reader` but no writer — so the
//! reflection bake hand-rolls the encode over the same binary layout the crate's `Reader`
//! validates and parses (`render::cubemap::parse_ktx2_cubemap`, #244). A KTX2 file is a
//! fixed 80-byte header, a level index (one 24-byte entry per mip, largest mip first), a
//! Basic Data Format Descriptor, then the level data. We emit exactly that for an LDR
//! RGBA8 cubemap with a full roughness mip chain, so the loader reads back what we wrote.
//!
//! Output is plain baked-asset bytes — a pure function of the prefiltered faces, no
//! wall-clock / RNG — so the encode is clean under the determinism guard.

use super::reflection_prefilter::PrefilteredCubemap;
use super::CubemapFace;

const MAGIC: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];
const HEADER_LEN: usize = 80;
const LEVEL_INDEX_ENTRY_LEN: usize = 24;
/// `VK_FORMAT_R8G8B8A8_SRGB` — the LDR sRGB format the bake emits and the loader accepts.
const VK_FORMAT_R8G8B8A8_SRGB: u32 = 43;

/// Encode a prefiltered cubemap into KTX2 bytes: an `R8G8B8A8_SRGB` cubemap with one level
/// per roughness mip (largest first), each level holding the six faces packed in
/// `CubemapFace::ALL` order. The bytes parse with `ktx2::Reader` / `parse_ktx2_cubemap`.
pub fn encode_cubemap(cube: &PrefilteredCubemap) -> Vec<u8> {
    debug_assert!(
        super::reflection_prefilter::validate_layout(cube),
        "prefiltered cube has a malformed mip layout"
    );
    let levels = cube.level_count();
    let dfd = build_dfd();
    let dfd_offset = HEADER_LEN + levels * LEVEL_INDEX_ENTRY_LEN;
    let level_data_offset = dfd_offset + dfd.len();

    // Lay out level data largest mip first; record each level's (offset, length).
    let mut level_bytes: Vec<u8> = Vec::new();
    let mut index: Vec<(u64, u64)> = Vec::with_capacity(levels);
    for level in 0..levels {
        let offset = level_data_offset as u64 + level_bytes.len() as u64;
        let start = level_bytes.len();
        for face in CubemapFace::ALL {
            level_bytes.extend_from_slice(&cube.mips[level][face as usize]);
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

/// Write the 80-byte KTX2 header for a `size × size` RGBA8 SRGB cubemap (`face_count = 6`,
/// no array layers, no supercompression) with `levels` mips and the given DFD location.
fn write_header(buf: &mut [u8], size: u32, levels: u32, dfd_offset: usize, dfd_len: usize) {
    buf[0..12].copy_from_slice(&MAGIC);
    put_u32(buf, 12, VK_FORMAT_R8G8B8A8_SRGB); // vkFormat
    put_u32(buf, 16, 1); // typeSize (1 byte per channel)
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

/// Write the level index: one 24-byte entry per mip (offset, length, uncompressed length;
/// uncompressed == length, no supercompression). Entry 0 is the largest mip.
fn write_level_index(buf: &mut [u8], index: &[(u64, u64)]) {
    for (i, (offset, length)) in index.iter().enumerate() {
        let at = HEADER_LEN + i * LEVEL_INDEX_ENTRY_LEN;
        put_u64(buf, at, *offset);
        put_u64(buf, at + 8, *length);
        put_u64(buf, at + 16, *length);
    }
}

/// A minimal Basic Data Format Descriptor for `R8G8B8A8_SRGB`: the 4-byte total-size word
/// followed by a 24-byte basic-descriptor block header. The loader only checks the format
/// enum (from the header) and the DFD's presence/bounds, so a compact-but-valid block —
/// total size word + an empty basic block — satisfies `ktx2::Reader`'s integrity check.
fn build_dfd() -> Vec<u8> {
    // descriptorType=0 (basic), vendorId=0, versionNumber=2, descriptorBlockSize=24.
    let block_size: u32 = 24;
    let total: u32 = 4 + block_size;
    let mut dfd = Vec::with_capacity(total as usize);
    dfd.extend_from_slice(&total.to_le_bytes()); // dfdTotalSize
                                                 // descriptorBlock word 0: vendorId(17) | descriptorType(15)
    dfd.extend_from_slice(&0u32.to_le_bytes());
    // word 1: versionNumber(16) | descriptorBlockSize(16)
    dfd.extend_from_slice(&((block_size << 16) | 2).to_le_bytes());
    // words 2..6: colorModel/primaries/transfer/flags, texel block dims, bytesPlanes —
    // zeroed; the loader doesn't read them. Pads the block to descriptorBlockSize bytes.
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
