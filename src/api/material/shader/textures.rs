//! src/api/material/shader/textures.rs — `Material.SetShaderTexture` (#400): the
//! texture an extra shader slot samples (a baked noise behind `dissolve`), Unity's
//! `material.SetTexture("_Mask", …)`.
//!
//! A thin adapter over `authoring::material::set_shader_texture`, which refuses an
//! unknown slot by name. `nil` or `""` clears the slot back to white.

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scene::authoring::material as mat_ops;
use crate::scene::Scene;

/// Register `SetShaderTexture` onto the `Material` `table`.
pub fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetShaderTexture",
        lua.create_function(
            move |lua, (id, slot, path): (EntityArg, String, Option<String>)| {
                let scene = &*scene.get(lua)?;
                let id = id.live("Material", "SetShaderTexture")?;
                let mut scene = scene.borrow_mut();
                let key = mat_ops::ensure_material_key(&mut scene, id)
                    .ok_or_else(|| mlua::Error::RuntimeError(format!("no entity {id}")))?;
                mat_ops::set_shader_texture(&mut scene.materials, &key, &slot, path)
                    .map_err(mlua::Error::RuntimeError)
            },
        ),
    )
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::scene::authoring::{create_entity, Primitive};

    fn run(scene: &Rc<RefCell<Scene>>, script: &str) -> mlua::Result<()> {
        let lua = Lua::new();
        crate::api::attempt(|| {
            crate::api::material::register(&lua, crate::api::bind(&lua, scene)).unwrap();
            lua.load(script).exec()
        })
    }

    #[test]
    fn set_shader_texture_names_clears_and_refuses_unknown_slots() {
        let mut scene = Scene::new();
        let id = create_entity(&mut scene, "Ball", Some(Primitive::Sphere));
        let scene = Rc::new(RefCell::new(scene));
        run(
            &scene,
            &format!(r#"Material.SetShaderTexture({id}, "mask", "n.png")"#),
        )
        .unwrap();
        let key = crate::scene::authoring::material::entity_material_key(id);
        let mask = |s: &Rc<RefCell<Scene>>| {
            s.borrow().materials[&key]
                .shader_textures
                .get("mask")
                .cloned()
        };
        assert_eq!(mask(&scene), Some("n.png".into()));

        run(
            &scene,
            &format!(r#"Material.SetShaderTexture({id}, "mask", nil)"#),
        )
        .unwrap();
        assert_eq!(mask(&scene), None, "nil clears to white");

        let err = run(
            &scene,
            &format!(r#"Material.SetShaderTexture({id}, "maks", "n.png")"#),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("\"maks\"") && err.contains("mask"), "{err}");
    }

    #[test]
    fn define_asset_takes_shader_textures_and_refuses_unknown_slots() {
        let scene = Rc::new(RefCell::new(Scene::new()));
        run(
            &scene,
            r#"Material.DefineAsset("burn", { shader = "die", shader_textures = { mask = "n.png" } })"#,
        )
        .unwrap();
        let mask = scene.borrow().materials["burn"].shader_textures["mask"].clone();
        assert_eq!(mask, "n.png");

        let err = run(
            &scene,
            r#"Material.DefineAsset("bad", { shader_textures = { detail = "n.png" } })"#,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("\"detail\""), "{err}");
        assert!(
            !scene.borrow().materials.contains_key("bad"),
            "nothing defined"
        );
    }
}
