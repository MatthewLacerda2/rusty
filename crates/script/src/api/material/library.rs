//! `Material.DeleteAsset` (#1576): take a material out of the scene's library, as
//! deleting a material asset in Unity does, through the shared
//! `authoring::material::delete_asset` op the editor's Material card calls too.

use super::{put, Reg};
use crate::api::ApiCell;
use rusty_scene::authoring::material as mat_ops;
use rusty_scene::Scene;

/// Register `DeleteAsset` onto the `Material` `table`.
pub fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "DeleteAsset",
        lua.create_function(move |lua, (name, opts): (String, Option<mlua::Table>)| {
            let scene = &*scene.get(lua)?;
            let force = match opts {
                Some(opts) => opts.get::<Option<bool>>("force")?.unwrap_or(false),
                None => false,
            };
            let mut scene = scene.borrow_mut();
            let deleted = mat_ops::delete_asset(&mut scene, &name, force)
                .map_err(mlua::Error::RuntimeError)?;
            let moved = deleted.moved.iter();
            Ok(moved
                .map(|u| mat_ops::describe(&scene, u))
                .collect::<Vec<_>>())
        }),
    )
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use rusty_components::MaterialComponent;

    /// Register the `Material` namespace against `scene` and evaluate `script`.
    fn eval<T: mlua::FromLuaMulti>(scene: &Rc<RefCell<Scene>>, script: &str) -> mlua::Result<T> {
        let lua = Lua::new();
        crate::api::attempt(|| {
            super::super::register(&lua, crate::api::bind(&lua, scene)).unwrap();
            lua.load(script).eval()
        })
    }

    #[test]
    fn delete_asset_refuses_a_used_material_unless_forced() {
        let scene = Rc::new(RefCell::new(Scene::new()));
        let id = scene.borrow_mut().add_entity("Pool".into());
        scene
            .borrow_mut()
            .materials
            .insert("checker".into(), Default::default());
        let used = Some(MaterialComponent::new("checker"));
        scene.borrow_mut().world.set_material(id, used);

        let err = eval::<()>(&scene, r#"Material.DeleteAsset("checker")"#).unwrap_err();
        assert!(err.to_string().contains("\"Pool\" slot 0"), "{err}");
        assert!(scene.borrow().materials.contains_key("checker"));

        let script = r#"return Material.DeleteAsset("checker", { force = true })"#;
        let moved: Vec<String> = eval(&scene, script).unwrap();
        assert_eq!(moved, [format!("entity {id} \"Pool\" slot 0")]);
        let gone = eval::<bool>(&scene, r#"return Material.HasAsset("checker")"#).unwrap();
        assert!(!gone);
        let first = scene
            .borrow()
            .world
            .material(id)
            .unwrap()
            .first()
            .to_string();
        assert_eq!(first, mat_ops::entity_material_key(id));
    }

    #[test]
    fn delete_asset_of_an_unused_material_returns_no_users() {
        let scene = Rc::new(RefCell::new(Scene::new()));
        scene
            .borrow_mut()
            .materials
            .insert("deck".into(), Default::default());
        let moved: Vec<String> = eval(&scene, r#"return Material.DeleteAsset("deck")"#).unwrap();
        assert!(moved.is_empty());
        assert!(eval::<()>(&scene, r#"Material.DeleteAsset("deck")"#).is_err());
    }
}
