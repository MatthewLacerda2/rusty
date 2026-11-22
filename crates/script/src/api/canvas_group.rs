use mlua::Lua;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::CanvasGroupComponent;
use rusty_edit::authoring::ui::layout::canvas_group as group_ops;
use rusty_scene::Scene;

type BoolGet = fn(&CanvasGroupComponent) -> bool;
type BoolSet = fn(&mut CanvasGroupComponent, bool);

const BOOLS: [(&str, BoolGet, BoolSet); 2] = [
    (
        "Interactable",
        |g| g.interactable,
        group_ops::set_interactable,
    ),
    (
        "BlocksRaycasts",
        |g| g.blocks_raycasts,
        group_ops::set_blocks_raycasts,
    ),
];

pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    put(
        &table,
        "GetAlpha",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene.world.canvas_group(id.read()).map_or(1.0, |g| g.alpha))
        }),
    )?;
    put(
        &table,
        "SetAlpha",
        lua.create_function(move |lua, (id, alpha): (EntityArg, f32)| {
            let id = id.live("CanvasGroup", "SetAlpha")?;
            if let Some(mut g) = scene.get(lua)?.borrow_mut().world.canvas_group_mut(id) {
                group_ops::set_alpha(&mut g, alpha);
            }
            Ok(())
        }),
    )?;
    for (suffix, get, set) in BOOLS {
        register_bool(lua, &table, scene, suffix, get, set)?;
    }
    lua.globals()
        .set("CanvasGroup", table)
        .map_err(|e| e.to_string())
}

fn register_bool(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    suffix: &str,
    get: BoolGet,
    set: BoolSet,
) -> Reg {
    put(
        table,
        &format!("Get{suffix}"),
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene.world.canvas_group(id.read()).is_some_and(|g| get(&g)))
        }),
    )?;
    let verb = format!("Set{suffix}");
    put(
        table,
        &format!("Set{suffix}"),
        lua.create_function(move |lua, (id, value): (EntityArg, bool)| {
            let id = id.live("CanvasGroup", &verb)?;
            if let Some(mut g) = scene.get(lua)?.borrow_mut().world.canvas_group_mut(id) {
                set(&mut g, value);
            }
            Ok(())
        }),
    )
}
