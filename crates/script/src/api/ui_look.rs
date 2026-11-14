//! crates/script/src/api/ui_look.rs — the look accessors every UI graphic namespace shares (#425).
//!
//! `Get/SetBlend` (a mode by name: `Normal`, `Additive`, `Multiply`, `Screen`) on
//! `Image`, `Text` and `Shape`, and `Get/SetGradient` on `Image` and `Shape`. A
//! gradient travels as a table — `{ kind = "Linear"|"Radial", angle = deg,
//! center = {x, y}, radius = r, stops = { { t = 0, color = {r, g, b, a} }, … } }`
//! (omitted keys default) — or the same document as a JSON string; `nil` clears
//! it. Writes route through the namespace's shared `scene::authoring` op, which
//! sanitizes the gradient once (`ui_look::sanitize_gradient`).

use mlua::{Table, Value};

use super::lua_json::{recipe_from_lua, to_lua};
use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::{UiBlend, UiGradient};
use rusty_scene::Scene;

/// How a namespace reads and writes its component's blend mode.
pub(super) struct BlendAccess {
    pub(super) get: fn(&Scene, u32) -> Option<UiBlend>,
    pub(super) set: fn(&mut Scene, u32, UiBlend),
}

/// How a namespace reads and writes its component's gradient (`None` outside:
/// no component; inside: no gradient).
pub(super) struct GradientAccess {
    pub(super) get: fn(&Scene, u32) -> Option<Option<UiGradient>>,
    pub(super) set: fn(&mut Scene, u32, Option<UiGradient>),
}

/// `Get/SetBlend` on `table`. The getter returns `"None"` without the component;
/// an unknown name is ignored.
pub(super) fn register_blend(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    (namespace, access): (&'static str, BlendAccess),
) -> Reg {
    let BlendAccess { get, set } = access;
    put(
        table,
        "GetBlend",
        lua.create_function(move |lua, id: EntityArg| {
            let blend = get(&scene.get(lua)?.borrow(), id.read());
            Ok(blend.map_or("None", UiBlend::name).to_string())
        }),
    )?;
    put(
        table,
        "SetBlend",
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            let id = id.live(namespace, "SetBlend")?;
            if let Some(blend) = UiBlend::parse(&name) {
                set(&mut scene.get(lua)?.borrow_mut(), id, blend);
            }
            Ok(())
        }),
    )
}

/// `Get/SetGradient` on `table`: see the module docs for the table's shape.
pub(super) fn register_gradient(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    (namespace, access): (&'static str, GradientAccess),
) -> Reg {
    let GradientAccess { get, set } = access;
    put(
        table,
        "GetGradient",
        lua.create_function(move |lua, id: EntityArg| {
            let Some(Some(g)) = get(&scene.get(lua)?.borrow(), id.read()) else {
                return Ok(Value::Nil);
            };
            to_lua(lua, &g)
        }),
    )?;
    put(
        table,
        "SetGradient",
        lua.create_function(move |lua, (id, value): (EntityArg, Value)| {
            let id = id.live(namespace, "SetGradient")?;
            let g = match value {
                Value::Nil => None,
                v => Some(recipe_from_lua::<UiGradient>(&v).map_err(mlua::Error::RuntimeError)?),
            };
            set(&mut scene.get(lua)?.borrow_mut(), id, g);
            Ok(())
        }),
    )
}
