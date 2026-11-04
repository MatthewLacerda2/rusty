//! What a lightmap page uploads as (#1694): a KTX2 page as BC6H with its mips where
//! the GPU has BC, else its PNG decoded to `Rgba16Float` with the same mips; a
//! scene baked before (a `.png` page) still binds; a windowed renderer reads off
//! its thread and binds once the read lands. Skips when no adapter is present.

use std::path::Path;

use glam::Vec3;

use super::Lightmaps;
use crate::asset::lightmap_page;
use crate::render::test_gpu::headless_or_skip;
use crate::scene::lighting::lightmap::{colour_page, encode_texels, MIP_LEVELS};

/// A `edge`² page of smooth light written as `name.ktx2` and its `name.png`
/// fallback; returns the KTX2 path.
fn page(name: &str, edge: u32) -> String {
    let dir = crate::test_temp::dir().join("rusty_1694");
    std::fs::create_dir_all(&dir).unwrap();
    let light: Vec<Vec3> = (0..edge * edge)
        .map(|i| Vec3::new(0.5 + (i % edge) as f32 / edge as f32, 1.0, 9.0))
        .collect();
    let ktx2 = dir.join(format!("{name}.ktx2"));
    let page = colour_page(&light, edge, 2);
    std::fs::write(&ktx2, lightmap_page::write(&page)).unwrap();
    image::RgbaImage::from_raw(edge, edge, encode_texels(&light))
        .unwrap()
        .save(ktx2.with_extension("png"))
        .unwrap();
    ktx2.to_string_lossy().into_owned()
}

#[test]
fn gpu_a_ktx2_page_binds_compressed_with_its_mips_or_from_its_png() {
    let Some(renderer) = headless_or_skip(4, 4) else {
        return;
    };
    let (device, queue) = (&renderer.device, &renderer.queue);
    let bc = device
        .features()
        .contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
    let pages = vec![page("a", 64), page("b", 64)];
    let mut maps = Lightmaps::new(device, true);
    assert!(maps.bind(device, queue, &pages, &[]));
    let format = match bc {
        true => wgpu::TextureFormat::Bc6hRgbUfloat,
        false => wgpu::TextureFormat::Rgba16Float,
    };
    assert_eq!(maps.bound_format(), Some((format, MIP_LEVELS)));
    let texel = format.block_copy_size(None).unwrap() as u64 / if bc { 16 } else { 1 };
    let base = 64 * 64 * texel * 2;
    assert_eq!(
        maps.gpu_bytes(),
        base + base / 4 + base / 16,
        "two pages, three levels"
    );

    // A scene baked before #1694 references its PNGs: they bind as linear light.
    let png = vec![Path::new(&pages[0])
        .with_extension("png")
        .to_string_lossy()
        .into_owned()];
    assert!(maps.bind(device, queue, &png, &[]));
    assert_eq!(
        maps.bound_format(),
        Some((wgpu::TextureFormat::Rgba16Float, MIP_LEVELS))
    );
}

#[test]
fn gpu_a_windowed_renderer_binds_once_the_read_lands() {
    let Some(renderer) = headless_or_skip(4, 4) else {
        return;
    };
    let (device, queue) = (&renderer.device, &renderer.queue);
    let pages = vec![page("late", 32)];
    let mut maps = Lightmaps::new(device, false);
    maps.bind(device, queue, &pages, &[]);
    for _ in 0..2000 {
        if maps.resident() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
        maps.bind(device, queue, &pages, &[]);
    }
    assert!(maps.resident(), "the worker's read landed and bound");
    assert!(!maps.bind(device, queue, &pages, &[]), "and then holds");
}
