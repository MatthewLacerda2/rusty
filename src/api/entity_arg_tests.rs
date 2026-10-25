use mlua::{FromLua, Lua, Value};

use super::EntityArg;
use crate::ecs::{World, NO_ENTITY};

fn arg(lua: &Lua, src: &str) -> mlua::Result<EntityArg> {
    lua.load(src).eval::<EntityArg>()
}

#[test]
fn an_integer_is_the_id_and_nil_is_none() {
    let lua = Lua::new();
    assert_eq!(arg(&lua, "7").map(EntityArg::get).ok(), Some(Some(7)));
    assert_eq!(arg(&lua, "7.0").map(EntityArg::get).ok(), Some(Some(7)));
    assert_eq!(arg(&lua, "nil").map(EntityArg::get).ok(), Some(None));
}

#[test]
fn what_is_not_an_id_is_still_a_conversion_error() {
    let lua = Lua::new();
    assert!(arg(&lua, "'seven'").is_err());
    assert!(arg(&lua, "-1").is_err());
    assert!(arg(&lua, "{}").is_err());
    assert!(<EntityArg as FromLua>::from_lua(Value::Boolean(true), &lua).is_err());
}

#[test]
fn a_read_of_nil_names_no_entity() {
    assert_eq!(EntityArg(None).read(), NO_ENTITY);
    assert_eq!(EntityArg::from(3).read(), 3);
    let mut world = World::new();
    let first = world.spawn("a".into());
    assert_ne!(first, NO_ENTITY);
    assert!(!world.contains(NO_ENTITY));
}

#[test]
fn a_write_of_nil_is_an_error_naming_the_verb() {
    let err = EntityArg(None).live("Scene", "SetActive");
    let message = err.map_err(|e| e.to_string()).err();
    assert_eq!(
        message.as_deref(),
        Some("runtime error: Scene.SetActive: no entity (nil id)")
    );
    assert_eq!(EntityArg::from(4).live("Scene", "SetActive").ok(), Some(4));
}
