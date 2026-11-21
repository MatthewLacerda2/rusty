use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use crate::api::bind;
use rusty_scene::Scene;

fn eval<T: mlua::FromLuaMulti>(scene: &Rc<RefCell<Scene>>, script: &str) -> mlua::Result<T> {
    let lua = Lua::new();
    crate::api::attempt(|| {
        super::register(&lua, bind(&lua, scene)).map_err(mlua::Error::RuntimeError)?;
        lua.load(script).eval()
    })
}

#[test]
fn a_detail_mask_sets_reads_back_and_clears_through_lua_and_a_recipe() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let id = scene.borrow_mut().add_entity("E".into());
    let (none, set, cleared): (Option<String>, String, Option<String>) = eval(
        &scene,
        &format!(
            r#"local none = Material.GetDetailMask({id})
               Material.SetDetailMap({id}, "d.png") Material.SetDetailMask({id}, "m.png")
               local set = Material.GetDetailMask({id})
               Material.SetDetailMask({id}, "")
               return none, set, Material.GetDetailMask({id})"#
        ),
    )
    .unwrap();
    assert_eq!((none, set.as_str(), cleared), (None, "m.png", None));
    eval::<()>(
        &scene,
        r#"Material.DefineAsset("tiles", { detail = { map = "d.png", mask = "m.png" } })"#,
    )
    .unwrap();
    let detail = scene.borrow().materials["tiles"].detail.clone().unwrap();
    assert_eq!(detail.mask.map(|m| m.to_string()).as_deref(), Some("m.png"));
}

#[test]
fn a_recipes_empty_mask_reads_back_as_no_mask() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let id = scene.borrow_mut().add_entity("E".into());
    let mask: Option<String> = eval(
        &scene,
        &format!(
            r#"Material.DefineAsset("bare", {{ detail = {{ map = "d.png", mask = "" }} }})
               Material.Set({id}, "bare")
               return Material.GetDetailMask({id})"#
        ),
    )
    .unwrap();
    assert_eq!(mask, None);
    assert_eq!(
        scene.borrow().materials["bare"]
            .detail
            .clone()
            .unwrap()
            .mask,
        None
    );
}
