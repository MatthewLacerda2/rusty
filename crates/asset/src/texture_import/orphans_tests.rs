use std::path::Path;

use super::orphans::{collect, orphans};
use super::*;

fn touch(dir: &Path, rel: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"").unwrap();
}

#[test]
fn caches_of_gone_sources_and_of_baked_pages_are_orphans_and_the_rest_stay() {
    let root = rusty_test_support::temp::dir();
    let (cache, sources) = (root.join("cache"), root.join("assets"));
    touch(&sources, "floor.png");
    touch(&sources, "a.scene.lightmaps/lightmap_0_00000001.png");
    touch(&cache, "floor.png.bctex");
    touch(&cache, "gone.png.bctex");
    touch(&cache, "deep/gone_too.png.bctex");
    touch(&cache, "a.scene.lightmaps/lightmap_0_00000001.png.bctex");
    touch(&cache, "a.scene.lightmaps/lightmap_0_ffffffff.png.bctex");
    touch(&cache, "model.fbx.glb");
    let found: Vec<String> = orphans(&cache, &sources)
        .iter()
        .map(|p| rusty_core::paths::to_slash(p.strip_prefix(&cache).unwrap()))
        .collect();
    assert_eq!(
        found,
        [
            "a.scene.lightmaps/lightmap_0_00000001.png.bctex",
            "a.scene.lightmaps/lightmap_0_ffffffff.png.bctex",
            "deep/gone_too.png.bctex",
            "gone.png.bctex",
        ],
        "a model's cache and a live image's stay"
    );
}

/// A path under a file answers `ENOTDIR`: unreadable, not "not found".
#[test]
fn a_source_that_cannot_be_read_is_not_gone() {
    let root = rusty_test_support::temp::dir();
    let (cache, sources) = (root.join("cache"), root.join("assets"));
    touch(&sources, "blocked.png");
    touch(&cache, "blocked.png/x.png.bctex");
    assert!(orphans(&cache, &sources).is_empty());
}

#[test]
fn outside_a_project_nothing_is_collected() {
    let root = rusty_test_support::temp::dir();
    touch(&root, "gone.png.bctex");
    assert_eq!(
        collect(&root),
        0,
        "the cache lives in the temp folder there"
    );
    assert!(root.join("gone.png.bctex").is_file());
}

#[test]
fn a_baked_page_is_not_an_image_to_compress() {
    let root = rusty_test_support::temp::dir();
    let page = root
        .join("s.scene.lightmaps")
        .join("lightmap_0_00000001.png");
    std::fs::create_dir_all(page.parent().unwrap()).unwrap();
    image::RgbaImage::new(8, 8).save(&page).unwrap();
    let floor = root.join("floor.png");
    image::RgbaImage::new(8, 8).save(&floor).unwrap();
    assert_eq!(images(&root), [floor]);
}
