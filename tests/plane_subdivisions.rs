//! A `Plane`'s subdivisions (#1524): the grid a vertex block (`vertex_waves`) moves,
//! set from Lua through `Component.Set`, refused out of range or on another shape,
//! and saved with the mesh.

use std::cell::RefCell;
use std::rc::Rc;

use rusty::scene::Scene;

use crate::play::{playing, run};

#[test]
fn a_subdivided_plane_is_set_from_lua_and_round_trips_through_save() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let mut game = playing(scene.clone(), 1.0);
    run(&mut game, 1);
    let eval = |line: &str| game.script_manager().eval(line);
    let id: u32 = eval(
        r#"local id = Scene.CreateEntity("Water", "Plane")
        Component.Set(id, "Mesh", { subdivisions = 64 })
        return id"#,
    )
    .unwrap()
    .parse()
    .unwrap();
    let vertices = |s: &Scene| s.world.mesh(id).map(|m| (m.subdivisions, m.vertices.len()));
    assert_eq!(vertices(&scene.borrow()), Some((64, 65 * 65)));

    // Saved from edit mode (Play refuses a save): the same mesh, in a scene of its own.
    let mut edit = Scene::new();
    let copy = edit.add_entity("Water".to_string());
    let mesh = scene.borrow().world.mesh(id).map(|m| m.clone());
    edit.world.set_mesh(copy, mesh);
    let path = crate::temp::dir().join("rusty_plane_subdivisions.scene");
    edit.save_to_file(&path.to_string_lossy()).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path.to_string_lossy()).unwrap();
    let vertices = |s: &Scene| {
        s.world
            .mesh(copy)
            .map(|m| (m.subdivisions, m.vertices.len()))
    };
    assert_eq!(
        vertices(&loaded),
        Some((64, 65 * 65)),
        "rehydrated as saved"
    );
}

#[test]
fn lua_refuses_a_bad_subdivision_count() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let mut game = playing(scene.clone(), 1.0);
    run(&mut game, 1);
    let eval = |line: &str| game.script_manager().eval(line);
    eval(r#"plane = Scene.CreateEntity("Water", "Plane"); cube = Scene.CreateEntity("Crate", "Box")"#)
        .unwrap();
    for (call, says) in [
        (
            r#"Component.Set(plane, "Mesh", { subdivisions = 0 })"#,
            "1..=256",
        ),
        (
            r#"Component.Set(plane, "Mesh", { subdivisions = 257 })"#,
            "1..=256",
        ),
        (
            r#"Component.Set(cube, "Mesh", { subdivisions = 8 })"#,
            "Plane",
        ),
    ] {
        let e = eval(call).unwrap_err();
        assert!(e.contains(says), "{call}: {e}");
    }
    let plane = scene.borrow().world.ids().to_vec()[0];
    let mesh = scene.borrow().world.mesh(plane).map(|m| m.vertices.len());
    assert_eq!(mesh, Some(4), "a refused count leaves the single quad");
}
