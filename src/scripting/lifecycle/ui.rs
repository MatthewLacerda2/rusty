//! src/scripting/lifecycle/ui.rs — dispatch the UI callbacks (#420).
//!
//! The event system (`ui::events`) decides *who* receives each UI event; this file
//! answers its one question about scripts — does entity `id` have a live script
//! defining a callback ([`ScriptManager::has_ui_handler`], the bubbling test) — and
//! fires what it returns ([`ScriptManager::dispatch_ui_events`]), in order, inside
//! one API scope. Direct calls, no event bus. The same active gate as every other
//! gameplay hook applies: only awoken instances of active entities are called.

use glam::Vec2;
use mlua::{Lua, Table};

use super::super::callbacks::{
    ON_BEGIN_DRAG, ON_CANCEL, ON_DESELECT, ON_DRAG, ON_END_DRAG, ON_MOVE, ON_POINTER_CLICK,
    ON_POINTER_DOWN, ON_POINTER_ENTER, ON_POINTER_EXIT, ON_POINTER_UP, ON_SCROLL, ON_SELECT,
    ON_SUBMIT,
};
use super::super::manager::ScriptManager;
use crate::ui::events::{Delivery, PointerEvent, UiHook};

/// The callback name a UI hook dispatches as.
fn callback(hook: UiHook) -> &'static str {
    match hook {
        UiHook::PointerEnter => ON_POINTER_ENTER,
        UiHook::PointerExit => ON_POINTER_EXIT,
        UiHook::PointerDown => ON_POINTER_DOWN,
        UiHook::PointerUp => ON_POINTER_UP,
        UiHook::PointerClick => ON_POINTER_CLICK,
        UiHook::BeginDrag => ON_BEGIN_DRAG,
        UiHook::Drag => ON_DRAG,
        UiHook::EndDrag => ON_END_DRAG,
        UiHook::Scroll => ON_SCROLL,
        UiHook::Select => ON_SELECT,
        UiHook::Deselect => ON_DESELECT,
        UiHook::Submit => ON_SUBMIT,
        UiHook::Cancel => ON_CANCEL,
        UiHook::Move => ON_MOVE,
    }
}

impl ScriptManager {
    /// Whether active entity `id` has an awoken script defining `hook`'s callback.
    pub fn has_ui_handler(&self, id: u32, hook: UiHook) -> bool {
        let Some(lua) = &self.lua else {
            return false;
        };
        let name = callback(hook);
        self.awoken_keys_for(id).into_iter().any(|key| {
            self.entity_scripts
                .get(&key)
                .and_then(|inst| lua.registry_value::<Table>(&inst.table).ok())
                .is_some_and(|t| t.get::<mlua::Function>(name).is_ok())
        })
    }

    /// Fire `deliveries` in order: each on every awoken script of its (still active)
    /// entity, in script-index order. The pointer family gets `(id, event)`,
    /// `OnMove` `(id, { direction, x, y })`, the focus family `(id)`.
    pub fn dispatch_ui_events(&self, deliveries: &[Delivery]) {
        if deliveries.is_empty() {
            return;
        }
        self.with_api_scope(|lua| {
            for d in deliveries {
                let name = callback(d.hook);
                for key in self.awoken_keys_for(d.entity) {
                    let table = |e: &PointerEvent| match d.hook {
                        UiHook::Move => move_table(lua, e),
                        _ => event_table(lua, e),
                    };
                    match d.event.as_ref().map(table) {
                        Some(Ok(event)) => self.call_hook(lua, key, name, (d.entity, event)),
                        Some(Err(_)) => {}
                        None => self.call_hook(lua, key, name, d.entity),
                    }
                }
            }
        });
    }
}

/// `{ direction, x, y }` — the Move's unit direction (y-up) and its name.
fn move_table(lua: &Lua, e: &PointerEvent) -> mlua::Result<Table> {
    let d = e.delta;
    let name = match (d.x as i32, d.y as i32) {
        (0, 1) => "Up",
        (0, _) => "Down",
        (-1, _) => "Left",
        _ => "Right",
    };
    let t = lua.create_table()?;
    t.set("direction", name)?;
    t.set("x", d.x)?;
    t.set("y", d.y)?;
    Ok(t)
}

/// `{ button, position = {x, y}, delta = {x, y}, target, canvas_position = {x, y}
/// (absent off the canvas), canvas_delta = {x, y} }`.
fn event_table(lua: &Lua, e: &PointerEvent) -> mlua::Result<Table> {
    let point = |v: Vec2| -> mlua::Result<Table> {
        let t = lua.create_table()?;
        t.set("x", v.x)?;
        t.set("y", v.y)?;
        Ok(t)
    };
    let t = lua.create_table()?;
    t.set("button", e.button.name())?;
    t.set("position", point(e.position)?)?;
    t.set("delta", point(e.delta)?)?;
    t.set("target", e.target)?;
    if let Some(p) = e.canvas_position {
        t.set("canvas_position", point(p)?)?;
    }
    t.set("canvas_delta", point(e.canvas_delta)?)?;
    Ok(t)
}
