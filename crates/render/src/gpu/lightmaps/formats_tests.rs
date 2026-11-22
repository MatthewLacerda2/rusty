use std::path::Path;

use glam::Vec3;

use super::Lightmaps;
use crate::test_gpu::headless_or_skip;
use rusty_asset::lightmap_page;
use rusty_lighting::lightmap::{colour_page, encode_texels};
use rusty_scene::lighting::page::MIP_LEVELS;

fn page(name: &str, edge: u32) -> String {
    let dir = rusty_test_support::temp::dir().join("rusty_1694");
    std::fs::create_dir_all(&dir).unwrap();
    let light: Vec<Vec3> = (0..edge * edge)
        .map(|i| Vec3::new(0.5 + (i % edge) as f32 / edge as f32, 1.0, 9.0))
        .collect();
    let ktx2 = dir.join(format!("{name}.ktx2"));
    let page = colour_page(&light, edge, 2, &|| false).expect("never stopped");
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
    let (device, writes) = (&renderer.device, (&renderer.queue, &renderer.staging));
    let bc = device
        .features()
        .contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
    let pages = vec![page("a", 64), page("b", 64)];
    let mut maps = Lightmaps::new(device, true);
    assert!(maps.bind(device, writes, &pages, &[], &[]));
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

    let ktx2_bytes = maps.gpu_bytes();
    let png = vec![Path::new(&pages[0])
        .with_extension("png")
        .to_string_lossy()
        .into_owned()];
    assert!(maps.bind(device, writes, &png, &[], &[]));
    assert_eq!(
        maps.bound_format(),
        Some((wgpu::TextureFormat::Rgba16Float, 1)),
        "its old layout carries no mips"
    );
    let png_bytes = 64 * 64 * 8;
    assert_eq!(
        maps.gpu_bytes(),
        ktx2_bytes + png_bytes,
        "every cached list"
    );
}

#[test]
fn gpu_a_windowed_renderer_binds_once_the_read_lands() {
    let Some(renderer) = headless_or_skip(4, 4) else {
        return;
    };
    let (device, writes) = (&renderer.device, (&renderer.queue, &renderer.staging));
    let pages = vec![page("late", 32)];
    let mut maps = Lightmaps::new(device, false);
    maps.bind(device, writes, &pages, &[], &[]);
    for _ in 0..2000 {
        if maps.resident() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
        maps.bind(device, writes, &pages, &[], &[]);
    }
    assert!(maps.resident(), "the worker's read landed and bound");
    assert!(
        !maps.bind(device, writes, &pages, &[], &[]),
        "and then holds"
    );
}

#[test]
fn gpu_a_loader_that_died_is_a_failed_load_not_pending_forever() {
    let Some(renderer) = headless_or_skip(4, 4) else {
        return;
    };
    let (device, writes) = (&renderer.device, (&renderer.queue, &renderer.staging));
    let mut maps = Lightmaps::new(device, false);
    let list = vec!["dead.ktx2".to_string()];
    let (outbox, inbox) = std::sync::mpsc::channel();
    drop(outbox);
    maps.cache
        .insert(list.clone(), super::Entry::Pending(inbox));
    maps.bind(device, writes, &list, &[], &[]);
    assert!(matches!(maps.cache[&list], super::Entry::Ready(None)));
    assert!(!maps.resident());
}
