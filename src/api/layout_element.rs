//! src/api/layout_element.rs — `LayoutElement` namespace (#421).
//!
//! Get/Set over an entity's `LayoutElementComponent` (Unity's `LayoutElement` +
//! `ContentSizeFitter`): `IgnoreLayout`, the min / preferred / flexible size
//! overrides as `(width, height)` pairs where `nil` means "use the content's
//! size", and the content fitter per axis (`Unconstrained` / `MinSize` /
//! `PreferredSize`). Every setter routes through the shared
//! `scene::authoring::layout_element` ops the inspector card uses. Getters return
//! a neutral default without the component; setters are then no-ops.

use std::cell::RefCell;

use mlua::{FromLuaMulti, IntoLuaMulti, Lua, Table};

use super::{put, Reg};
use crate::components::LayoutElementComponent;
use crate::scene::authoring::layout_element::{self as ops, SizeKind, FITS};
use crate::scene::authoring::layout_group::{name_of, parse};
use crate::scene::Scene;

type Cell<'s> = &'s RefCell<Scene>;
type E = LayoutElementComponent;
type Size = (Option<f32>, Option<f32>);

/// Register the `LayoutElement` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: Cell<'scope>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    let ignore = |e: &E| e.ignore_layout;
    pair(scope, &t, scene, "IgnoreLayout", ignore, |e, v: bool| {
        ops::set_ignore_layout(e, v)
    })?;
    for (suffix, kind) in [
        ("MinSize", SizeKind::Min),
        ("PreferredSize", SizeKind::Preferred),
        ("FlexibleSize", SizeKind::Flexible),
    ] {
        let get = move |e: &E| ops::size(e, kind);
        let set = move |e: &mut E, (w, h): Size| ops::set_size(e, kind, w, h);
        pair(scope, &t, scene, suffix, get, set)?;
    }
    let fit = |e: &E| {
        (
            name_of(&FITS, e.horizontal_fit),
            name_of(&FITS, e.vertical_fit),
        )
    };
    let set_fit = |e: &mut E, (h, v): (String, String)| {
        if let (Some(h), Some(v)) = (parse(&FITS, &h), parse(&FITS, &v)) {
            ops::set_fit(e, h, v);
        }
    };
    pair(scope, &t, scene, "Fit", fit, set_fit)?;
    lua.globals()
        .set("LayoutElement", t)
        .map_err(|e| e.to_string())
}

/// Register `Get<suffix>(id)` → `get(element)` (Lua's defaults without one) and
/// `Set<suffix>(id, …)` → `set(element, …)` (a no-op without one).
fn pair<'lua, 'scope, R, A>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: Cell<'scope>,
    suffix: &str,
    get: impl Fn(&E) -> R + 'scope,
    set: impl Fn(&mut E, A) + 'scope,
) -> Reg
where
    R: for<'a> IntoLuaMulti<'a> + Default,
    A: for<'a> FromLuaMulti<'a>,
{
    let f = scope.create_function(move |_, id: u32| {
        Ok(scene
            .borrow()
            .world
            .layout_element(id)
            .map(|e| get(&e))
            .unwrap_or_default())
    });
    put(t, &format!("Get{suffix}"), f)?;
    let f = scope.create_function(move |lua, (id, rest): (u32, mlua::MultiValue)| {
        let value = A::from_lua_multi(rest, lua)?;
        if let Some(mut e) = scene.borrow_mut().world.layout_element_mut(id) {
            set(&mut e, value);
        }
        Ok(())
    });
    put(t, &format!("Set{suffix}"), f)
}
