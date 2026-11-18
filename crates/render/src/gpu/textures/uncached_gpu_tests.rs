//! `Renderer::uncached_textures` (#2319): a map whose settings ask for compression
//! but that has no current import cache counts, with its RGBA8 bytes; once the
//! import made its cache, its upload is compressed and it no longer counts. A
//! map set to Compression None, too small to block-compress, inside the import
//! cache (extracted beside a converted model) or outside the imported folder (the
//! engine's own maps) never counts: no import would make it a cache.

use std::path::Path;

use image::{Rgba, RgbaImage};

use super::compressed::FEATURE;
use super::freshness_tests::temp_png;
use rusty_asset::sidecar::{set_texture, Compression, TextureSettings};
use rusty_asset::texture_import::refresh;
use rusty_asset::MapRef;

fn png(name: &str, side: u32) -> String {
    save(temp_png(name), side)
}

/// A wanted-compression PNG at `rel` under `base`, made fresh.
fn png_at(base: &Path, rel: &str) -> String {
    let path = base.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    save(path.to_string_lossy().into_owned(), 16)
}

fn save(path: String, side: u32) -> String {
    RgbaImage::from_pixel(side, side, Rgba([9, 99, 199, 255]))
        .save(&path)
        .unwrap();
    // Outside a project the cache sits in the system temp dir: a dead run's may be there.
    let cache = rusty_asset::texture_import::cache_file(Path::new(&path)).unwrap();
    let _ = std::fs::remove_file(cache);
    path
}

#[test]
fn gpu_a_map_drawn_uncompressed_for_want_of_its_cache_counts_until_imported() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let wanted = png("uncached_wanted", 16);
    let raw = png("uncached_raw", 16);
    let none = TextureSettings {
        compression: Compression::None,
        ..Default::default()
    };
    set_texture(Path::new(&raw), none).unwrap();
    let odd = png("uncached_odd", 6);
    let root = Path::new(&wanted).parent().unwrap().to_path_buf();
    let cache = rusty_asset::convert::IMPORT_CACHE_DIR;
    let extracted = png_at(&root, &format!("{cache}/uncached_extracted.png"));
    let elsewhere = rusty_test_support::temp::dir().join("rusty_2319_outside");
    let outside = png_at(&elsewhere, "uncached_outside.png");
    let maps = [&wanted, &raw, &odd, &extracted, &outside];
    for map in maps {
        renderer.load_texture(&MapRef::from(map.as_str()));
    }
    let found = renderer.uncached_textures(&root);
    if !renderer.device.features().contains(FEATURE) {
        assert_eq!(found.count, 0, "no BC here: an import would not help");
        return;
    }
    assert_eq!(found.count, 1, "only the map an import would compress");
    // 16² + 8² + 4² + 2² + 1² texels at four bytes.
    assert_eq!(found.bytes, 341 * 4);
    refresh(Path::new(&wanted), "").unwrap();
    renderer.gpu_textures.clear();
    for map in maps {
        renderer.load_texture(&MapRef::from(map.as_str()));
    }
    assert_eq!(
        renderer.uncached_textures(&root).count,
        0,
        "imported: compressed"
    );
}
