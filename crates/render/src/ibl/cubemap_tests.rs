use super::*;

pub(super) fn build_cubemap_ktx2(size: u32, format: u32, face_count: u32) -> Vec<u8> {
    const MAGIC: [u8; 12] = [
        0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
    ];
    let texel = match format {
        BC6H_UFLOAT => 1,
        R16G16B16A16_SFLOAT => 8,
        _ => 4,
    };
    let face_bytes = match format {
        BC6H_UFLOAT => rusty_asset::texture_import::blocks_len(size, size, 16),
        _ => (size as usize) * (size as usize) * texel,
    };
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
    put(&mut buf, 16, (texel as u32 / 4).max(1)); // typeSize
    put(&mut buf, 20, size); // pixelWidth
    put(&mut buf, 24, size); // pixelHeight
    put(&mut buf, 28, 0); // pixelDepth
    put(&mut buf, 32, 0); // layerCount
    put(&mut buf, 36, face_count); // faceCount
    put(&mut buf, 40, 1); // levelCount
    put(&mut buf, 44, 0); // supercompressionScheme
    put(&mut buf, 48, dfd_offset as u32); // dfdByteOffset
    put(&mut buf, 52, dfd_len as u32); // dfdByteLength

    let li = header_len;
    buf[li..li + 8].copy_from_slice(&(level_offset as u64).to_le_bytes());
    buf[li + 8..li + 16].copy_from_slice(&(level_len as u64).to_le_bytes());
    buf[li + 16..li + 24].copy_from_slice(&(level_len as u64).to_le_bytes());

    for f in 0..face_count as usize {
        buf[level_offset + f * face_bytes] = f as u8;
    }
    buf
}

pub(super) const R8G8B8A8_UNORM: u32 = 37;
pub(super) const R8G8B8A8_SRGB: u32 = 43;
pub(super) const R16G16B16A16_SFLOAT: u32 = 97;
pub(super) const BC6H_UFLOAT: u32 = 143;
const R32G32B32A32_SFLOAT: u32 = 109;

#[test]
fn parses_valid_bc6h_cubemap() {
    let bytes = build_cubemap_ktx2(8, BC6H_UFLOAT, 6);
    let cube = parse_ktx2_cubemap_mips(&bytes).expect("should parse");
    assert_eq!(cube.size, 8);
    assert_eq!(cube.mips[0].len(), 4 * 16 * 6);
    let face_bytes = 4 * 16;
    assert_eq!(cube.mips[0][0], 0);
    assert_eq!(cube.mips[0][face_bytes], 1);
    assert_eq!(cube.mips[0][5 * face_bytes], 5);
}

#[test]
fn a_cube_from_before_bc6h_probes_asks_for_a_rebake() {
    for format in [R8G8B8A8_UNORM, R8G8B8A8_SRGB, R16G16B16A16_SFLOAT] {
        let why = parse_ktx2_cubemap_mips(&build_cubemap_ktx2(1, format, 6)).err();
        assert!(why.is_some_and(|w| w.contains("re-bake")), "{why:?}");
    }
}

#[test]
fn rejects_non_cube() {
    let bytes = build_cubemap_ktx2(4, BC6H_UFLOAT, 1);
    assert!(parse_ktx2_cubemap_mips(&bytes).is_err());
}

#[test]
fn rejects_another_float_format() {
    let bytes = build_cubemap_ktx2(1, R32G32B32A32_SFLOAT, 6);
    assert_eq!(
        parse_ktx2_cubemap_mips(&bytes).err(),
        Some("it is not BC6H")
    );
}

#[test]
fn rejects_garbage() {
    assert!(parse_ktx2_cubemap_mips(b"not a ktx2 file at all").is_err());
    assert!(parse_ktx2_cubemap_mips(&[]).is_err());
}

#[test]
fn gpu_rebaking_a_resident_cubemap_recopies_it() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(4, 4) else {
        return;
    };
    let path = rusty_test_support::temp::dir().join("bound.ktx2");
    let write = || {
        let bytes = build_cubemap_ktx2(4, BC6H_UFLOAT, 6);
        rusty_core::fs::written::write(&path, &bytes).unwrap();
    };
    write();
    let mut scene = rusty_scene::Scene::new();
    let i = scene
        .reflection_probes
        .add(glam::Vec3::ZERO, glam::Vec3::ONE);
    let key = path.to_string_lossy().into_owned();
    scene.reflection_probes.set_cubemap(i, key);
    renderer.update_reflection_probes(&scene, glam::Vec3::ZERO);
    renderer.update_reflection_probes(&scene, glam::Vec3::ZERO);
    assert_eq!(
        renderer.reflection_probes.copies, 1,
        "a resident cube stays"
    );
    renderer.global_bind_group_dirty = false;
    write();
    renderer.refresh_textures();
    renderer.update_reflection_probes(&scene, glam::Vec3::ZERO);
    assert_eq!(
        renderer.reflection_probes.copies, 2,
        "the re-bake is copied in"
    );
    assert!(!renderer.global_bind_group_dirty, "same size: same array");
}
