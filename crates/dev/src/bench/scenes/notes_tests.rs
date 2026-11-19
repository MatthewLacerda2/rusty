//! The warnings beside a bench report: textures drawn uncompressed for want of an
//! import (#2319), and lightmap pages a scene names that are not on disk.

use super::uncompressed;

#[test]
fn textures_drawn_uncompressed_for_want_of_an_import_are_warned_of_with_their_size() {
    assert_eq!(uncompressed(rusty_render::Uploads::default()), None);
    let one = rusty_render::Uploads {
        count: 1,
        bytes: 341 * 4,
    };
    let w = uncompressed(one).unwrap();
    assert!(
        w.starts_with("uploaded uncompressed: 1 texture, 0.0 MB"),
        "{w}"
    );
    let many = rusty_render::Uploads {
        count: 3,
        bytes: 3 << 20,
    };
    let w = uncompressed(many).unwrap();
    assert!(w.contains("3 textures, 3.0 MB"), "{w}");
    assert!(w.contains("make import"), "{w}");
}

/// A scene that names its lightmap pages but whose pages are not on disk (a copy of
/// the game without its bake) is warned of, with how many are missing (#2420).
#[test]
fn lightmap_pages_named_but_not_on_disk_are_counted() {
    use super::unbaked;
    use rusty_scene::lighting::lightmap_set::LightmapEntry;
    use rusty_scene::Scene;
    let dir = rusty_test_support::temp::dir().join(format!("notes_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let here = dir.join("page_0.ktx2");
    std::fs::write(&here, b"page").unwrap();
    let mut scene = Scene::new();
    scene.lightmaps.insert(LightmapEntry {
        entity: 1,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    });
    let gone = dir.join("page_1.ktx2").to_string_lossy().into_owned();
    scene.lightmaps.pages = vec![here.to_string_lossy().into_owned(), gone.clone()];
    let w = unbaked(&scene).unwrap();
    assert!(w.contains("lightmaps (1 of 2 pages not on disk)"), "{w}");
    scene.lightmaps.directions = vec![gone.clone(), gone];
    let w = unbaked(&scene).unwrap();
    assert!(
        w.contains("lightmaps (1 of 2 pages, 2 of 2 direction pages not on disk)"),
        "{w}"
    );
    scene.lightmaps.pages.pop();
    scene.lightmaps.directions.clear();
    assert_eq!(unbaked(&scene), None, "every page on disk: no note");
}
