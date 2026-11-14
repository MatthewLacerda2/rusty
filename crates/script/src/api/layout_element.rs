//! crates/script/src/api/layout_element.rs — `LayoutElement` namespace (#421).
//!
//! Get/Set over an entity's `LayoutElementComponent` (Unity's `LayoutElement` +
//! `ContentSizeFitter`): `IgnoreLayout`, the min / preferred / flexible size
//! overrides as `(width, height)` pairs where `nil` means "use the content's
//! size", and the content fitter per axis (`Unconstrained` / `MinSize` /
//! `PreferredSize`). Every setter routes through the shared
//! `scene::authoring::ui::layout::element` ops the inspector card uses. Getters return
//! a neutral default without the component; setters are then no-ops.

use mlua::{FromLuaMulti, IntoLuaMulti, Lua, Table};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::{LayoutAxisFit, LayoutElementComponent};
use rusty_scene::authoring::ui::layout::element::{self as ops, SizeKind};
use rusty_scene::Scene;

type E = LayoutElementComponent;
type Size = (Option<f32>, Option<f32>);

/// Register the `LayoutElement` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    let ignore = |e: &E| e.ignore_layout;
    pair(lua, &t, scene, "IgnoreLayout", ignore, |e, v: bool| {
        ops::set_ignore_layout(e, v)
    })?;
    for (suffix, kind) in [
        ("MinSize", SizeKind::Min),
        ("PreferredSize", SizeKind::Preferred),
        ("FlexibleSize", SizeKind::Flexible),
    ] {
        let get = move |e: &E| ops::size(e, kind);
        let set = move |e: &mut E, (w, h): Size| ops::set_size(e, kind, w, h);
        pair(lua, &t, scene, suffix, get, set)?;
    }
    let fit = |e: &E| (e.horizontal_fit.name(), e.vertical_fit.name());
    let set_fit = |e: &mut E, (h, v): (String, String)| {
        if let (Some(h), Some(v)) = (LayoutAxisFit::parse(&h), LayoutAxisFit::parse(&v)) {
            ops::set_fit(e, h, v);
        }
    };
    pair(lua, &t, scene, "Fit", fit, set_fit)?;
    lua.globals()
        .set("LayoutElement", t)
        .map_err(|e| e.to_string())
}

/// Register `Get<suffix>(id)` → `get(element)` (Lua's defaults without one) and
/// `Set<suffix>(id, …)` → `set(element, …)` (a no-op without one).
fn pair<R, A>(
    lua: &mlua::Lua,
    t: &Table,
    scene: ApiCell<Scene>,
    suffix: &str,
    get: impl Fn(&E) -> R + 'static,
    set: impl Fn(&mut E, A) + 'static,
) -> Reg
where
    R: IntoLuaMulti + Default,
    A: FromLuaMulti,
{
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(scene
            .get(lua)?
            .borrow()
            .world
            .layout_element(id.read())
            .map(|e| get(&e))
            .unwrap_or_default())
    });
    put(t, &format!("Get{suffix}"), f)?;
    let verb = format!("Set{suffix}");
    let f = lua.create_function(move |lua, (id, rest): (EntityArg, mlua::MultiValue)| {
        let id = id.live("LayoutElement", &verb)?;
        let value = A::from_lua_multi(rest, lua)?;
        if let Some(mut e) = scene.get(lua)?.borrow_mut().world.layout_element_mut(id) {
            set(&mut e, value);
        }
        Ok(())
    });
    put(t, &format!("Set{suffix}"), f)
}
