use super::super::session::Session;

fn session() -> Session {
    Session::new("").expect("an empty session boots")
}

#[test]
fn debug_is_on_the_surface() {
    let s = session();
    let surface = s.world().script_manager().api_surface();
    let debug = surface.get("Debug").expect("`Debug` is installed");
    assert!(debug.contains("Snapshot") && debug.contains("Stats"));
}

#[test]
fn every_bake_verb_is_installed_on_its_namespace() {
    let surface = session().world().script_manager().api_surface();
    for ns in ["Lighting", "Reflection", "Probe"] {
        assert!(surface[ns].contains("Bake"), "`{ns}.Bake` is missing");
    }
}

#[test]
fn installing_twice_adds_nothing() {
    super::install_api();
    super::install_api();
    let s = session();
    assert_eq!(s.eval("return type(Debug.Log)").unwrap(), "function");
}

#[test]
fn lighting_bake_on_an_empty_scene_returns_a_bool() {
    let out = session().eval("return Lighting.Bake()").unwrap();
    assert!(out == "true" || out == "false", "{out}");
}

#[test]
fn gpu_probe_bake_returns_a_bool_and_skips_gracefully() {
    let s = session();
    s.eval("Probe.FillGrid(0,0,0, 1,1,1, 1)").unwrap();
    let out = s.eval("return Probe.Bake()").unwrap();
    assert!(out == "true" || out == "false", "{out}");
}

#[test]
fn reflection_bake_with_no_probes_succeeds() {
    let s = session();
    *s.world().script_manager().scene_path_cell().borrow_mut() = Some("scene.scene".into());
    assert_eq!(s.eval("return Reflection.Bake()").unwrap(), "true");
}

#[test]
fn reflection_bake_needs_a_saved_scene() {
    let s = session();
    s.eval("Reflection.Add(0,0,0, 1,1,1)").unwrap();
    assert!(s.eval("return Reflection.Bake()").is_err());
}

#[test]
fn lightmap_bake_needs_a_saved_scene_and_counts_what_it_wrote() {
    let s = session();
    assert!(
        s.eval("return Lighting.BakeLightmaps()").is_err(),
        "unsaved scene"
    );
    let dir = rusty_test_support::temp::dir().join("rusty_lua_lightmaps");
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    *s.world().script_manager().scene_path_cell().borrow_mut() = Some(path);
    assert_eq!(
        s.eval("return Lighting.BakeLightmaps(2, 4, 1, 9)").unwrap(),
        "0"
    );
    assert_eq!(
        s.eval("return Lighting.BakeLightmaps(2, 4, 1, 9, false)")
            .unwrap(),
        "0"
    );
    s.eval("Lighting.ClearLightmaps()").unwrap();
}
