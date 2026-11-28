use std::path::Path;

use super::gpu_tests::{boxes, dropped, gradient, settle, RES};
use crate::gpu::textures::compressed::FEATURE;

#[test]
fn gpu_a_streamed_down_texture_is_audited_at_the_size_it_loads() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    r.stream_textures(true);
    let map = gradient("stream_audited", 64);
    let file = map.file().expect("a file");
    let cache = rusty_asset::texture_import::cache_file(&file).expect("a cache path");
    let _ = std::fs::remove_file(cache);
    settle(&mut r, &boxes(&[(&map, 150.0)]), RES, &[&map]);
    assert!(dropped(&r, &map) >= 4, "far: down to a few texels a side");
    if !r.device.features().contains(FEATURE) {
        return;
    }
    let root = file.parent().map(Path::to_path_buf).expect("a folder");
    let found = r.uncached_textures(&root);
    assert_eq!(found.count, 1, "still the one an import would compress");
    assert_eq!(found.bytes, 5461 * 4, "64² RGBA8 with every mip, as loaded");
}
