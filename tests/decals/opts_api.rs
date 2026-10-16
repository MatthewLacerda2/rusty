//! `Decals.Spawn`'s `opts` table form (#638): the positional values by name, plus
//! `depth` and the decal `material`; unknown options and materials are errors.

use std::cell::RefCell;

use mlua::Lua;
use rusty::components::MaterialAsset;
use rusty::scene::Scene;

fn run(scene: &RefCell<Scene>, script: &str) -> mlua::Result<()> {
    let lua = Lua::new();
    lua.scope(|scope| {
        rusty::api::decals::register(&lua, scope, scene).unwrap();
        lua.load(script).exec()
    })
}

fn with_blood() -> RefCell<Scene> {
    let mut scene = Scene::new();
    scene
        .materials
        .insert("blood".into(), MaterialAsset::default());
    RefCell::new(scene)
}

#[test]
fn opts_name_the_material_and_every_positional_value() {
    let scene = with_blood();
    run(
        &scene,
        r#"Decals.Spawn(0, 0, 0, 0, 1, 0, { material = "blood", size = 2, depth = 0.25,
            rotation = 30, color = {1, 0, 0, 0.5}, texture = "splat.png" })"#,
    )
    .unwrap();
    let s = scene.borrow();
    let d = &s.decals[0];
    assert_eq!(d.material.as_deref(), Some("blood"));
    assert_eq!((d.size.x, d.size.z), (2.0, 0.25), "size and explicit depth");
    assert_eq!(d.color, [1.0, 0.0, 0.0, 0.5]);
    assert_eq!(d.texture.as_deref(), Some("splat.png"));
}

#[test]
fn empty_opts_take_the_positional_defaults() {
    let scene = with_blood();
    run(&scene, "Decals.Spawn(0, 0, 0, 0, 1, 0, {})").unwrap();
    let s = scene.borrow();
    let d = &s.decals[0];
    assert_eq!((d.size.x, d.size.z), (0.5, 0.5));
    assert_eq!(d.color, [1.0; 4]);
    assert!(d.material.is_none() && d.texture.is_none());
}

#[test]
fn an_unknown_option_or_material_is_an_error_and_stamps_nothing() {
    let scene = with_blood();
    let typo = run(
        &scene,
        "Decals.Spawn(0, 0, 0, 0, 1, 0, { matrial = 'blood' })",
    );
    let err = typo.unwrap_err().to_string();
    assert!(err.contains("matrial") && err.contains("material"), "{err}");
    let missing = run(
        &scene,
        "Decals.Spawn(0, 0, 0, 0, 1, 0, { material = 'oil' })",
    );
    assert!(missing.unwrap_err().to_string().contains("oil"));
    assert!(scene.borrow().decals.is_empty());
}
