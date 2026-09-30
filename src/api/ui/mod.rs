//! src/api/ui/ — `UI` namespace: EventSystem-level UI verbs (#417, #420).
//!
//! Reads the in-game UI's computed layout. `UI.GetRect(id)` returns an element's
//! rect in its canvas's reference units and in screen pixels, computed on demand
//! from the live scene (so it reflects a change made earlier in the same script, in
//! edit mode as well as in Play) with the same math as the per-tick layout system.
//! The pointer and focus verbs (#420) are in `events`.

mod events;

pub use events::register_events;

use std::cell::RefCell;

use glam::Vec2;
use mlua::{Lua, Table};
use serde_json::{json, Value};

use super::{put, Reg};
use crate::core::video::VideoSettings;
use crate::scene::Scene;
use crate::ui::{layout, ScreenSize, UiRect};

/// Register the `UI` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    screen: &'scope RefCell<ScreenSize>,
    video: &'scope RefCell<VideoSettings>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    put(
        &table,
        "GetRect",
        scope.create_function(|lua, id: u32| {
            let px = screen.borrow().pixels(&video.borrow());
            let rect = layout::rect_of(&scene.borrow().world, id, px);
            rect.map(|r| rect_table(lua, &r)).transpose()
        }),
    )?;
    put(
        &table,
        "GetScreenSize",
        scope.create_function(|_, ()| {
            let px = screen.borrow().pixels(&video.borrow());
            Ok((px.x, px.y))
        }),
    )?;
    lua.globals().set("UI", table).map_err(|e| e.to_string())
}

/// `{ x, y, width, height }` of an axis-aligned `(min, max)` box.
fn box_table<'lua>(lua: &'lua Lua, (lo, hi): (Vec2, Vec2)) -> mlua::Result<Table<'lua>> {
    let t = lua.create_table()?;
    t.set("x", lo.x)?;
    t.set("y", lo.y)?;
    t.set("width", hi.x - lo.x)?;
    t.set("height", hi.y - lo.y)?;
    Ok(t)
}

/// The Lua shape of a rect — the same keys as [`rect_value`].
fn rect_table<'lua>(lua: &'lua Lua, r: &UiRect) -> mlua::Result<Table<'lua>> {
    let t = box_table(lua, r.bounds())?;
    t.set("screen", box_table(lua, r.screen_bounds())?)?;
    let corners = lua.create_table()?;
    for (i, c) in r.corners.iter().enumerate() {
        let p = lua.create_table()?;
        p.set("x", c.x)?;
        p.set("y", c.y)?;
        corners.set(i + 1, p)?;
    }
    t.set("corners", corners)?;
    t.set("canvas", r.canvas)?;
    t.set("scale_factor", r.scale_factor)?;
    Ok(t)
}

/// The JSON shape of a rect, for `Debug.Snapshot`'s `ui_rect`: the same keys as
/// `UI.GetRect`'s table.
pub(crate) fn rect_value(r: &UiRect) -> Value {
    let boxed = |(lo, hi): (Vec2, Vec2)| json!({ "x": lo.x, "y": lo.y, "width": hi.x - lo.x, "height": hi.y - lo.y });
    let mut v = boxed(r.bounds());
    v["screen"] = boxed(r.screen_bounds());
    v["corners"] = json!(r.corners.map(|c| json!({ "x": c.x, "y": c.y })));
    v["canvas"] = json!(r.canvas);
    v["scale_factor"] = json!(r.scale_factor);
    v
}
