//! Headless tests for the KTX2 cubemap loader (no GPU): a hand-built minimal cubemap
//! blob parses to six faces; garbage / non-cube / non-RGBA8 inputs are rejected.

use super::*;

/// Build a minimal-but-valid KTX2 byte blob for a `size × size` RGBA8 cubemap with one
/// mip level, each face a solid colour `face_index` in the red channel. Layout: 80-byte
/// header, one 24-byte level-index entry, a tiny DFD placeholder, then the packed faces.
fn build_cubemap_ktx2(size: u32, format: u32, face_count: u32) -> Vec<u8> {
    const MAGIC: [u8; 12] = [
        0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
    ];
    let face_bytes = (size as usize) * (size as usize) * 4;
    let level_len = face_bytes * face_count as usize;

    let header_len = 80usize;
    let level_index_len = 24usize; // one level
    let dfd_len = 4usize; // placeholder DFD (just its length word)
    let dfd_offset = header_len + level_index_len;
    let level_offset = dfd_offset + dfd_len;

    let mut buf = vec![0u8; level_offset + level_len];
    buf[0..12].copy_from_slice(&MAGIC);
    let put = |b: &mut [u8], at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut buf, 12, format); // vkFormat
    put(&mut buf, 16, 4); // typeSize
    put(&mut buf, 20, size); // pixelWidth
    put(&mut buf, 24, size); // pixelHeight
    put(&mut buf, 28, 0); // pixelDepth
    put(&mut buf, 32, 0); // layerCount
    put(&mut buf, 36, face_count); // faceCount
    put(&mut buf, 40, 1); // levelCount
    put(&mut buf, 44, 0); // supercompressionScheme
    put(&mut buf, 48, dfd_offset as u32); // dfdByteOffset
    put(&mut buf, 52, dfd_len as u32); // dfdByteLength

    // Level-index entry 0: offset, length, uncompressed length.
    let li = header_len;
    buf[li..li + 8].copy_from_slice(&(level_offset as u64).to_le_bytes());
    buf[li + 8..li + 16].copy_from_slice(&(level_len as u64).to_le_bytes());
    buf[li + 16..li + 24].copy_from_slice(&(level_len as u64).to_le_bytes());

    // Faces: tag each face's first pixel's red channel with its index so order is checkable.
    for f in 0..face_count as usize {
        buf[level_offset + f * face_bytes] = f as u8;
    }
    buf
}

const R8G8B8A8_UNORM: u32 = 37;
const R8G8B8A8_SRGB: u32 = 43;
const R16G16B16A16_SFLOAT: u32 = 97; // an HDR format we must reject

#[test]
fn parses_valid_rgba8_cubemap() {
    let bytes = build_cubemap_ktx2(2, R8G8B8A8_UNORM, 6);
    let cube = parse_ktx2_cubemap_mips(&bytes).expect("should parse");
    assert_eq!(cube.size, 2);
    assert_eq!(cube.mips[0].len(), 2 * 2 * 4 * 6);
    // Face order preserved: face f tagged with red = f at its start.
    let face_bytes = 2 * 2 * 4;
    assert_eq!(cube.mips[0][0], 0);
    assert_eq!(cube.mips[0][face_bytes], 1);
    assert_eq!(cube.mips[0][5 * face_bytes], 5);
}

#[test]
fn accepts_srgb_too() {
    let bytes = build_cubemap_ktx2(1, R8G8B8A8_SRGB, 6);
    assert!(parse_ktx2_cubemap_mips(&bytes).is_some());
}

#[test]
fn rejects_non_cube() {
    // A 2D texture (1 face) is not a cubemap.
    let bytes = build_cubemap_ktx2(1, R8G8B8A8_UNORM, 1);
    assert!(parse_ktx2_cubemap_mips(&bytes).is_none());
}

#[test]
fn rejects_hdr_format() {
    let bytes = build_cubemap_ktx2(1, R16G16B16A16_SFLOAT, 6);
    assert!(parse_ktx2_cubemap_mips(&bytes).is_none());
}

#[test]
fn rejects_garbage() {
    assert!(parse_ktx2_cubemap_mips(b"not a ktx2 file at all").is_none());
    assert!(parse_ktx2_cubemap_mips(&[]).is_none());
}

/// A reflection cubemap re-baked at a path the cache already loaded (or failed to)
/// is read again on the next frame (#1207).
#[test]
fn gpu_a_cubemap_rebaked_at_a_cached_path_reloads() {
    use crate::core::fs::written::contract::{follows_rebakes, Bake};
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(4, 4) else {
        return;
    };
    let size = |bake| if bake == Bake::A { 2 } else { 4 };
    let path = crate::test_temp::dir().join("probe.ktx2");
    follows_rebakes(
        &path.to_string_lossy(),
        |p, bake| {
            let bytes = build_cubemap_ktx2(size(bake), R8G8B8A8_SRGB, 6);
            crate::core::fs::written::write(std::path::Path::new(p), &bytes).unwrap();
        },
        |p| {
            renderer.refresh_textures();
            renderer.cached_cubemap(p).map(|cube| cube.size)
        },
        |loaded, bake| *loaded == Some(size(bake)),
    );
}

/// Re-baking the bound probe's cubemap unbinds it at once, so a camera that next
/// picks no probe does not keep sampling the old cube.
#[test]
fn gpu_rebaking_the_bound_cubemap_unbinds_it() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(4, 4) else {
        return;
    };
    let path = crate::test_temp::dir().join("bound.ktx2");
    let write = |size| {
        let bytes = build_cubemap_ktx2(size, R8G8B8A8_SRGB, 6);
        crate::core::fs::written::write(&path, &bytes).unwrap();
    };
    write(2);
    let key = path.to_string_lossy().into_owned();
    renderer.reflection_cube = renderer.cached_cubemap(&key);
    renderer.reflection_cube_path = key;
    renderer.global_bind_group_dirty = false;
    write(4);
    renderer.refresh_textures();
    assert!(
        renderer.reflection_cube.is_none(),
        "the old cube stays bound"
    );
    assert!(renderer.reflection_cube_path.is_empty());
    assert!(renderer.global_bind_group_dirty, "group 0 must rebind");
}
