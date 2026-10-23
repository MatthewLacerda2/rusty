//! `Material.Get` / `Set` / `GetAll` / `SetAt` (#1285): one slot per material run of
//! the entity's mesh; a bad slot or key is a Lua error, never a panic.

use std::cell::RefCell;

use mlua::Lua;

use crate::components::{MaterialAsset, MaterialComponent, MeshComponent};
use crate::scene::Scene;

/// Run `script` against `scene`, returning what it returns (or its error's text).
fn eval(scene: &RefCell<Scene>, script: &str) -> Result<String, String> {
    let lua = Lua::new();
    lua.scope(|s| {
        crate::api::material::register(&lua, s, scene).map_err(mlua::Error::RuntimeError)?;
        let value: mlua::Value = lua.load(script).eval()?;
        Ok(match value {
            mlua::Value::Table(t) => t
                .sequence_values::<String>()
                .collect::<mlua::Result<Vec<_>>>()?
                .join(","),
            other => other.to_string()?,
        })
    })
    .map_err(|e| e.to_string())
}

/// An entity whose mesh is two runs drawn with `red` and `blue`, and a third
/// library material `green`.
fn two_run_quad() -> (RefCell<Scene>, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Quad".into());
    let mesh = MeshComponent {
        ranges: vec![0..3, 3..6],
        ..MeshComponent::new("Asset", Vec::new(), vec![0, 1, 2, 0, 2, 3])
    };
    scene.world.set_mesh(id, Some(mesh));
    let both = MaterialComponent {
        materials: vec!["red".into(), "blue".into()],
    };
    scene.world.set_material(id, Some(both));
    for key in ["red", "blue", "green"] {
        scene.materials.insert(key.into(), MaterialAsset::default());
    }
    (RefCell::new(scene), id)
}

#[test]
fn get_all_lists_one_key_per_run_and_get_is_the_first() {
    let (scene, id) = two_run_quad();
    assert_eq!(
        eval(&scene, &format!("return Material.GetAll({id})")).unwrap(),
        "red,blue"
    );
    assert_eq!(
        eval(&scene, &format!("return Material.Get({id})")).unwrap(),
        "red"
    );
}

#[test]
fn set_at_changes_only_its_slot_and_set_the_first() {
    let (scene, id) = two_run_quad();
    eval(&scene, &format!(r#"Material.SetAt({id}, 1, "green")"#)).unwrap();
    assert_eq!(
        scene.borrow().world.material(id).unwrap().materials,
        ["red", "green"]
    );
    eval(&scene, &format!(r#"Material.Set({id}, "blue")"#)).unwrap();
    assert_eq!(
        scene.borrow().world.material(id).unwrap().materials,
        ["blue", "green"]
    );
}

#[test]
fn a_slot_out_of_range_or_an_unknown_key_is_an_error_that_changes_nothing() {
    let (scene, id) = two_run_quad();
    let out = eval(&scene, &format!(r#"Material.SetAt({id}, 3, "green")"#)).unwrap_err();
    assert!(out.contains("slot 3 is out of range"), "{out}");
    let out = eval(&scene, &format!(r#"Material.Set({id}, "chrome")"#)).unwrap_err();
    assert!(out.contains("no material asset \"chrome\""), "{out}");
    assert_eq!(
        scene.borrow().world.material(id).unwrap().materials,
        ["red", "blue"]
    );
}

#[test]
fn set_at_one_past_the_last_slot_adds_one_and_getall_round_trips_through_ipairs() {
    let (scene, id) = two_run_quad();
    eval(&scene, &format!(r#"Material.SetAt({id}, 2, "green")"#)).unwrap();
    let copy = format!(
        "for i, key in ipairs(Material.GetAll({id})) do Material.SetAt({id}, i - 1, key) end
         return Material.GetAll({id})"
    );
    assert_eq!(eval(&scene, &copy).unwrap(), "red,blue,green");
}

#[test]
fn an_entity_without_a_material_gets_one_from_set() {
    let scene = RefCell::new(Scene::new());
    let id = scene.borrow_mut().add_entity("E".into());
    scene
        .borrow_mut()
        .materials
        .insert("red".into(), MaterialAsset::default());
    assert_eq!(
        eval(&scene, &format!("return Material.Get({id})")).unwrap(),
        "nil"
    );
    eval(&scene, &format!(r#"Material.Set({id}, "red")"#)).unwrap();
    assert_eq!(
        eval(&scene, &format!("return Material.GetAll({id})")).unwrap(),
        "red"
    );
}
