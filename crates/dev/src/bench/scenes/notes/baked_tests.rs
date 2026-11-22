fn bake_lines(
    path: &str,
    refused: &[&str],
    stale: Option<rusty_render::StaleOcclusion>,
    nav: Option<rusty_navigation::Start>,
) -> Vec<String> {
    let scene = rusty_scene::Scene::new();
    for scene_path in refused {
        let pvs = rusty_scene::lighting::occlusion::occlusion_path(scene_path);
        scene.bake_files().refused(&pvs, b"bad", "occluder flag 30");
    }
    super::bakes_unused(&scene, path, stale, nav)
}

#[test]
fn baked_data_not_used_is_one_line_a_kind_naming_its_file_and_why() {
    let a = "levels/a.scene";
    assert_eq!(bake_lines(a, &[], None, None), Vec::<String>::new());
    let restored = Some(rusty_navigation::Start::Restored);
    assert_eq!(bake_lines(a, &[], None, restored), Vec::<String>::new());
    assert_eq!(
        bake_lines(a, &[a], None, None),
        ["baked occlusion not used: a.scene.occlusion refused (occluder flag 30)"]
    );
    let why = "stale: the scene changed".to_string();
    let lines = bake_lines(a, &[], None, Some(rusty_navigation::Start::Refused(why)));
    assert_eq!(
        lines,
        [
            "baked navmesh not used: a.scene.navmesh refused at Play's start (stale: the \
          scene changed), so Play baked it in full"
        ]
    );
}

#[test]
fn a_stale_pvs_is_unused_when_an_occluder_changed_and_partly_used_when_not() {
    use rusty_render::StaleOcclusion;
    let a = "levels/a.scene";
    let broken = StaleOcclusion {
        changed: 3,
        occluders: 1,
    };
    let line = &bake_lines(a, &[], Some(broken), None)[0];
    assert!(
        line.starts_with("baked occlusion not used: a.scene.occlusion stale (1 of the 3 "),
        "{line}"
    );
    let meshes = StaleOcclusion {
        changed: 2,
        occluders: 0,
    };
    let line = &bake_lines(a, &[], Some(meshes), None)[0];
    assert!(
        line.starts_with("baked occlusion partly used: a.scene.occlusion stale (2 baked meshes"),
        "{line}"
    );
    assert!(line.contains("always drawn"), "{line}");
}

#[test]
fn the_scene_loaded_before_in_the_same_world_lends_the_live_one_no_line() {
    let lines = bake_lines("levels/b.scene", &["levels/a.scene"], None, None);
    assert_eq!(lines, Vec::<String>::new(), "a's refusal is a's");
}

#[test]
fn a_sidecar_that_could_not_be_read_is_listed_with_its_error() {
    let scene = rusty_scene::Scene::new();
    let navmesh = rusty_scene::nav_settings::navmesh_path("a.scene");
    scene.bake_files().unreadable(&navmesh, "permission denied");
    assert_eq!(
        super::bakes_unused(&scene, "a.scene", None, None),
        ["baked navmesh not used: a.scene.navmesh could not be read (permission denied)"]
    );
}
