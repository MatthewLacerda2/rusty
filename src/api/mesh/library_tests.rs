//! Tests for the `Mesh` library verbs (#2059).

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use crate::scene::Scene;

fn lua_over(scene: &Rc<RefCell<Scene>>) -> Lua {
    let lua = Lua::new();
    super::super::register(&lua, crate::api::bind(&lua, scene)).unwrap();
    lua
}

const QUAD: &str = r#"Mesh.Create("quad", {
    positions = { 0,0,0,  1,0,0,  1,0,-1,  0,0,-1 },
    indices = { 0,1,2,  0,2,3 },
    uvs = { 0,0, 1,0, 1,1, 0,1 },
})"#;

#[test]
fn create_then_set_shared_mesh_draws_it_on_an_entity() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let id = scene.borrow_mut().add_entity("Floor".into());
    let lua = lua_over(&scene);
    lua.load(QUAD).exec().unwrap();
    lua.load(format!(r#"Mesh.SetSharedMesh({id}, "quad")"#))
        .exec()
        .unwrap();
    let got: (String, bool, bool) = lua
        .load(format!(
            r#"return Mesh.GetSharedMesh({id}), Mesh.HasAsset("quad"), Mesh.HasAsset("no")"#
        ))
        .eval()
        .unwrap();
    assert_eq!(got, ("quad".into(), true, false));
    let s = scene.borrow();
    let mesh = s.world.mesh(id).expect("a Mesh was added");
    assert_eq!((mesh.vertices.len(), mesh.indices.len()), (4, 6));
    assert_eq!(mesh.vertices[2].tex_coords, [1.0, 1.0]);
    assert_eq!(mesh.vertices[0].normal, [0.0, 1.0, 0.0]);
}

#[test]
fn get_asset_round_trips_the_flat_form() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let lua = lua_over(&scene);
    lua.load(QUAD).exec().unwrap();
    let copied: bool = lua
        .load(
            r#"local d = Mesh.GetAsset("quad")
               Mesh.Create("copy", d)
               return #d.positions == 12 and d.normals == nil and Mesh.GetAsset("none") == nil"#,
        )
        .eval()
        .unwrap();
    assert!(copied);
    let s = scene.borrow();
    assert_eq!(s.meshes["copy"], s.meshes["quad"]);
}

#[test]
fn replacing_a_mesh_redraws_every_entity_using_it_once() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let (a, b) = {
        let mut s = scene.borrow_mut();
        (s.add_entity("A".into()), s.add_entity("B".into()))
    };
    let lua = lua_over(&scene);
    lua.load(QUAD).exec().unwrap();
    lua.load(format!(
        r#"Mesh.SetSharedMesh({a}, "quad") Mesh.SetSharedMesh({b}, "quad")"#
    ))
    .exec()
    .unwrap();
    lua.load(
        r#"Mesh.Create("quad", { positions = { 0,0,0, 1,0,0, 0,1,0 }, indices = { 0,1,2 } })"#,
    )
    .exec()
    .unwrap();
    let s = scene.borrow();
    let (ma, mb) = (s.world.mesh(a).unwrap(), s.world.mesh(b).unwrap());
    assert_eq!((ma.vertices.len(), mb.indices.len()), (3, 3));
    assert_eq!(ma.revision, mb.revision, "one shared upload");
    assert_eq!(ma.revision, s.meshes["quad"].revision);
    assert_eq!(
        ma.vertices[0].normal,
        [0.0, 0.0, 1.0],
        "computed, facing +Z"
    );
}

#[test]
fn bad_input_is_a_lua_error_naming_the_verb() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let id = scene.borrow_mut().add_entity("E".into());
    let lua = lua_over(&scene);
    let error = |script: &str| lua.load(script).exec().expect_err(script).to_string();
    let cases = [
        (
            r#"Mesh.Create("m", { positions = { 0,0 }, indices = {} })"#,
            "x,y,z triples",
        ),
        (
            r#"Mesh.Create("m", { positions = { 0,0,0 }, indices = { 0,0 } })"#,
            "whole number of triangles",
        ),
        (
            r#"Mesh.Create("m", { positions = { 0,0,0 }, indices = { 0,0,1.5 } })"#,
            "index 2 is 1.5",
        ),
        (
            r#"Mesh.Create("m", { positions = { 0,0,0 }, indices = { 0,0,-1 } })"#,
            "index 2 is -1",
        ),
        (
            r#"Mesh.Create("m", { positions = { 0,0,0 }, indices = { 0,0,3 } })"#,
            "index 2 is 3",
        ),
        (
            r#"Mesh.Create("m", { positions = { 0,0/0,0 }, indices = {} })"#,
            "position 0",
        ),
        (
            r#"Mesh.Create("m", { positions = { 0,0,0 }, uvs = { 1 } })"#,
            "u,v pairs",
        ),
        (
            r#"Mesh.Create("Box", { positions = {} })"#,
            "a primitive's name",
        ),
        (r#"Mesh.Create("a::b", { positions = {} })"#, "separator"),
        (
            r#"Mesh.Create("m", { positions = { "x", 0, 0 } })"#,
            "positions",
        ),
        (
            &format!(r#"Mesh.SetSharedMesh({id}, "nope")"#),
            "no mesh named",
        ),
        (
            &format!(r#"Mesh.SetSharedMesh({id}, "PointLight")"#),
            "a light",
        ),
    ];
    for (script, expected) in cases {
        let message = error(script);
        assert!(message.contains(expected), "{script}: {message}");
    }
    assert!(
        scene.borrow().meshes.is_empty(),
        "nothing refused was defined"
    );
}

#[test]
fn set_shared_mesh_switches_back_to_a_primitive_keeping_the_settings() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let id = scene.borrow_mut().add_entity("E".into());
    let lua = lua_over(&scene);
    lua.load(QUAD).exec().unwrap();
    let script = format!(
        r#"Mesh.SetSharedMesh({id}, "quad")
           Mesh.SetMaxDrawDistance({id}, 12)
           Mesh.SetSharedMesh({id}, "sphere")
           return Mesh.GetSharedMesh({id}), Mesh.GetMaxDrawDistance({id})"#
    );
    let got: (String, f32) = lua.load(script).eval().unwrap();
    assert_eq!(got, ("Sphere".into(), 12.0));
    assert!(scene.borrow().world.mesh(id).unwrap().asset_ref.is_none());
}
