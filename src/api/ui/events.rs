//! src/api/ui/events.rs — the `UI` namespace's pointer and focus verbs (#420).
//!
//! Focus (`SetSelected` / `GetSelected`), the gameplay-vs-UI guards
//! (`IsPointerOverUI` / `IsPointerConsumed`), and the agent verbs: `Raycast` (what
//! is under a point, computed on demand from the live scene), `Click` (a real click
//! at an element's centre, through `Input`) and `List` (every visible Selectable,
//! with its state and screen rect). Points are UI screen pixels: bottom-left
//! origin, y-up — the frame of `UI.GetRect(id).screen`.

use glam::Vec2;
use mlua::{Lua, Table};

use super::super::{put, ApiScopedCtx, Reg};
use crate::ecs::World;
use crate::ui::events::{is_interactable, is_under, is_visible, raycast};
use crate::ui::layout::rect_of;
use crate::ui::{EventSystem, UiLayout};

/// Add the event verbs to the `UI` table.
pub(super) fn register<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table<'lua>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let (scene, input, events) = (ctx.scene, ctx.input, ctx.event_system);
    let (screen, video) = (ctx.screen, ctx.video);
    let px = move || screen.borrow().pixels(&video.borrow());
    let f = scope.create_function(move |_, id: Option<u32>| {
        events.borrow_mut().set_selected(id);
        Ok(())
    });
    put(table, "SetSelected", f)?;
    let f = scope.create_function(move |_, ()| Ok(events.borrow().selected()));
    put(table, "GetSelected", f)?;
    let f = scope.create_function(move |_, ()| Ok(events.borrow().is_pointer_over_ui()));
    put(table, "IsPointerOverUI", f)?;
    let f = scope.create_function(move |_, ()| Ok(events.borrow().is_pointer_consumed()));
    put(table, "IsPointerConsumed", f)?;
    let f = scope.create_function(move |_, (x, y): (f32, f32)| {
        let world = &scene.borrow().world;
        let layout = UiLayout::compute(world, px());
        Ok(raycast(world, &layout, Vec2::new(x, y)))
    });
    put(table, "Raycast", f)?;
    let f = scope.create_function(move |_, id: u32| {
        let screen = px();
        let world = &scene.borrow().world;
        let Some(centre) = rect_of(world, id, screen).map(|r| centre_px(&r)) else {
            return Ok(false);
        };
        let layout = UiLayout::compute(world, screen);
        let mut input = input.borrow_mut();
        let lands = !input.cursor().locked
            && raycast(world, &layout, centre).is_some_and(|h| is_under(world, h, id));
        input.move_mouse(f64::from(centre.x), f64::from(screen.y - centre.y));
        // Release first, so a held left button still yields a whole click.
        input.release("MOUSE0");
        input.press("MOUSE0");
        input.release("MOUSE0");
        Ok(lands)
    });
    put(table, "Click", f)?;
    let f = scope.create_function(move |lua, ()| {
        let world = &scene.borrow().world;
        list(
            lua,
            world,
            &UiLayout::compute(world, px()),
            &events.borrow(),
        )
    });
    put(table, "List", f)
}

/// The centre of an element's final quad, in screen pixels.
fn centre_px(r: &crate::ui::UiRect) -> Vec2 {
    r.corners.iter().copied().sum::<Vec2>() * 0.25 * r.scale_factor
}

/// `{ {id, name, state, interactable, selected, rect = {x, y, width, height}}, … }`
/// for every visible Selectable, in draw order; `rect` in screen pixels.
fn list<'lua>(
    lua: &'lua Lua,
    world: &World,
    layout: &UiLayout,
    events: &EventSystem,
) -> mlua::Result<Table<'lua>> {
    let out = lua.create_table()?;
    for (id, rect) in layout.iter() {
        if !world.has_selectable(id) || !is_visible(world, id) {
            continue;
        }
        let e = lua.create_table()?;
        e.set("id", id)?;
        e.set("name", world.name(id).map(|n| n.clone()))?;
        e.set("state", events.state_of(world, id).name())?;
        e.set("interactable", is_interactable(world, id))?;
        e.set("selected", events.selected() == Some(id))?;
        let (lo, hi) = rect.screen_bounds();
        let r = lua.create_table()?;
        r.set("x", lo.x)?;
        r.set("y", lo.y)?;
        r.set("width", hi.x - lo.x)?;
        r.set("height", hi.y - lo.y)?;
        e.set("rect", r)?;
        out.push(e)?;
    }
    Ok(out)
}
