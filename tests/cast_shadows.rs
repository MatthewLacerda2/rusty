//! A mesh's `cast_shadows` (#1571): set from Lua through `Component.Set`, read back
//! with `Component.Get`, refused for a name it doesn't know, and saved with the scene
//! and its prefabs.

use std::cell::RefCell;
use std::rc::Rc;

use rusty::components::ShadowCasting;
use rusty::scene::prefab::{instantiate_prefab, read_prefab_file, save_prefab};
use rusty::scene::Scene;

use crate::play::{playing, run};

#[test]
fn cast_shadows_is_set_from_lua_and_refused_when_unknown() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let mut game = playing(scene.clone(), 1.0);
    run(&mut game, 1);
    let eval = |line: &str| game.script_manager().eval(line);
    let read = eval(
        r#"water = Scene.CreateEntity("Water", "Plane")
        local before = Component.Get(water, "Mesh").cast_shadows
        Component.Set(water, "Mesh", { cast_shadows = "off" })
        return before .. " " .. Component.Get(water, "Mesh").cast_shadows"#,
    )
    .unwrap();
    assert_eq!(read, "On Off", "read in any case, stored by its name");
    let e = eval(r#"Component.Set(water, "Mesh", { cast_shadows = "Sometimes" })"#);
    assert!(
        e.unwrap_err().contains("ShadowsOnly"),
        "the error lists the modes"
    );
    let id = scene.borrow().world.ids()[0];
    let mode = scene.borrow().world.mesh(id).map(|m| m.cast_shadows);
    assert_eq!(
        mode,
        Some(ShadowCasting::Off),
        "a refused name changes nothing"
    );
}

#[test]
fn cast_shadows_round_trips_through_a_scene_and_a_prefab() {
    let mut scene = Scene::new();
    let [water, floor] = ["Water", "Floor"].map(|name| {
        let id = scene.add_entity(name.to_string());
        let mesh = rusty::scene::authoring::primitive_mesh_component(
            rusty::scene::authoring::Primitive::Plane,
        );
        scene.world.set_mesh(id, mesh);
        id
    });
    scene.world.mesh_mut(water).unwrap().cast_shadows = ShadowCasting::TwoSided;
    let dir = crate::temp::dir();
    let path = dir
        .join("rusty_cast_shadows.scene")
        .to_string_lossy()
        .into_owned();
    scene.save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    let mode = |s: &Scene, id| s.world.mesh(id).map(|m| m.cast_shadows);
    assert_eq!(mode(&loaded, water), Some(ShadowCasting::TwoSided));
    assert_eq!(mode(&loaded, floor), Some(ShadowCasting::On));

    let prefab = dir
        .join("rusty_cast_shadows.prefab")
        .to_string_lossy()
        .into_owned();
    save_prefab(&scene, water, &prefab).unwrap();
    let mut stamped = Scene::new();
    let root = instantiate_prefab(&mut stamped, &read_prefab_file(&prefab).unwrap(), None);
    assert_eq!(mode(&stamped, root), Some(ShadowCasting::TwoSided));
}
