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

#[test]
fn a_scene_whose_loads_cooked_its_models_is_warned_of() {
    use super::super::load::Load;
    let load = |cooked: f64| Load {
        steps: Vec::new(),
        counts: vec![("models_cooked".to_string(), cooked)],
        ..Load::default()
    };
    assert_eq!(super::cooked_at_load(&[load(0.0), load(0.0)]), None);
    assert_eq!(super::cooked_at_load(&[]), None);
    let w = super::cooked_at_load(&[load(0.0), load(3.0), load(1.0)]).unwrap();
    assert!(
        w.starts_with("its models were cooked at load (3 in a load)"),
        "{w}"
    );
    assert!(w.contains("make import"), "{w}");
}

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
