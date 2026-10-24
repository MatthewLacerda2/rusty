//! src/api/ui_look.rs — the look accessors every UI graphic namespace shares (#425).
//!
//! `Get/SetBlend` (a mode by name: `Normal`, `Additive`, `Multiply`, `Screen`) on
//! `Image`, `Text` and `Shape`, and `Get/SetGradient` on `Image` and `Shape`. A
//! gradient travels as a table — `{ kind = "Linear"|"Radial", angle = deg,
//! center = {x, y}, radius = r, stops = { { t = 0, color = {r, g, b, a} }, … } }`
//! (omitted keys default) — or the same document as a JSON string; `nil` clears
//! it. Writes route through the namespace's shared `scene::authoring` op, which
//! sanitizes the gradient once (`ui_look::sanitize_gradient`).

use std::cell::RefCell;

use mlua::{Table, Value};

use super::lua_json::{recipe_from_lua, to_lua};
use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::components::{UiBlend, UiGradient};
use crate::scene::Scene;

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
pub(super) fn register_blend<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    (namespace, access): (&'static str, BlendAccess),
) -> Reg {
    let BlendAccess { get, set } = access;
    put(
        table,
        "GetBlend",
        scope.create_function(move |_, id: EntityArg| {
            let id = id.read();
            let blend = get(&scene.borrow(), id);
            Ok(blend.map_or("None", UiBlend::name).to_string())
        }),
    )?;
    put(
        table,
        "SetBlend",
        scope.create_function(move |_, (id, name): (EntityArg, String)| {
            let id = id.live(namespace, "SetBlend")?;
            if let Some(blend) = UiBlend::parse(&name) {
                set(&mut scene.borrow_mut(), id, blend);
            }
            Ok(())
        }),
    )
}

/// `Get/SetGradient` on `table`: see the module docs for the table's shape.
pub(super) fn register_gradient<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    (namespace, access): (&'static str, GradientAccess),
) -> Reg {
    let GradientAccess { get, set } = access;
    put(
        table,
        "GetGradient",
        scope.create_function(move |lua, id: EntityArg| {
            let id = id.read();
            let Some(Some(g)) = get(&scene.borrow(), id) else {
                return Ok(Value::Nil);
            };
            to_lua(lua, &g)
        }),
    )?;
    put(
        table,
        "SetGradient",
        scope.create_function(move |_, (id, value): (EntityArg, Value)| {
            let id = id.live(namespace, "SetGradient")?;
            let g = match value {
                Value::Nil => None,
                v => Some(recipe_from_lua::<UiGradient>(&v).map_err(mlua::Error::RuntimeError)?),
            };
            set(&mut scene.borrow_mut(), id, g);
            Ok(())
        }),
    )
}
