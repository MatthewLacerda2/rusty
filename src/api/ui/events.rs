//! src/api/ui/events.rs — the `UI` namespace's pointer and focus verbs (#420).
//!
//! Focus (`SetSelected` / `GetSelected`), the gameplay-vs-UI guards
//! (`IsPointerOverUI` / `IsPointerConsumed`), and the agent verbs: `Raycast` (what
//! is under a point, computed on demand from the live scene), `FindSelectable`
//! (where navigation from an element would go — for an `OnMove` handler), `Click` (a real click
//! at an element's centre, through `Input`) and `List` (every visible Selectable,
//! with its state and screen rect). Points are UI screen pixels: bottom-left
//! origin, y-up — the frame of `UI.GetRect(id).screen`. Each point also casts the
//! camera ray through it, so world canvases answer too (#429).

use glam::Vec2;
use mlua::{Lua, Table};

use super::super::{global_table, put, ApiScopedCtx, Reg};
use super::view::{pointer, ui_view};
use crate::ecs::World;
use crate::ui::events::{find_selectable, is_interactable, is_under, is_visible, raycast_pointer};
use crate::ui::space::{canvas_to_screen, rect_screen_bounds};
use crate::ui::{EventSystem, UiLayout, UiRect, UiView};

/// Add the event verbs to the `UI` table [`super::register`] created.
pub fn register_events<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let table = &global_table(lua, "UI")?;
    let (scene, input, events) = (ctx.scene, ctx.input, ctx.event_system);
    let (screen, video, camera, physics) = (ctx.screen, ctx.video, ctx.camera, ctx.physics);
    let view = move || ui_view(screen, video, camera);
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
        let (view, world) = (view(), &scene.borrow().world);
        let layout = UiLayout::compute_in(world, &view);
        let at = pointer(&view, physics, Some(Vec2::new(x, y)));
        Ok(raycast_pointer(world, &layout, &at))
    });
    put(table, "Raycast", f)?;
    register_find(scope, table, ctx)?;
    let f = scope.create_function(move |_, id: u32| {
        let (view, world) = (view(), &scene.borrow().world);
        let layout = UiLayout::compute_in(world, &view);
        let Some(centre) = layout.get(id).and_then(|r| centre_px(&layout, r, &view)) else {
            return Ok(false);
        };
        let mut input = input.borrow_mut();
        // A locked cursor clicks through the screen centre (world canvases only).
        let locked = input.cursor().locked;
        let at = pointer(&view, physics, (!locked).then_some(centre));
        let lands = raycast_pointer(world, &layout, &at).is_some_and(|h| is_under(world, h, id));
        if !locked {
            input.move_mouse(f64::from(centre.x), f64::from(view.screen.y - centre.y));
        }
        // Release first, so a held left button still yields a whole click.
        input.release("MOUSE0");
        input.press("MOUSE0");
        input.release("MOUSE0");
        Ok(lands)
    });
    put(table, "Click", f)?;
    let f = scope.create_function(move |lua, ()| {
        let (view, world) = (view(), &scene.borrow().world);
        let layout = UiLayout::compute_in(world, &view);
        list(lua, world, (&layout, &view), &events.borrow())
    });
    put(table, "List", f)
}

/// `FindSelectable(id, direction)` — where navigation from `id` would go.
fn register_find<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let (scene, screen, video, camera) = (ctx.scene, ctx.screen, ctx.video, ctx.camera);
    let f = scope.create_function(move |_, (id, dir): (u32, String)| {
        let Some(dir) = direction(&dir) else {
            return Err(mlua::Error::RuntimeError(format!(
                "UI.FindSelectable: unknown direction '{dir}' (Up, Down, Left, Right)"
            )));
        };
        let world = &scene.borrow().world;
        let layout = UiLayout::compute_in(world, &ui_view(screen, video, camera));
        Ok(find_selectable(world, &layout, id, dir))
    });
    put(table, "FindSelectable", f)
}

/// A direction name as its `select_on` index (case-insensitive).
fn direction(name: &str) -> Option<usize> {
    ["up", "down", "left", "right"]
        .iter()
        .position(|d| d.eq_ignore_ascii_case(name))
}

/// The centre of an element's final quad, in screen pixels (`None`: a world
/// canvas behind the camera).
fn centre_px(layout: &UiLayout, r: &UiRect, view: &UiView) -> Option<Vec2> {
    let centre = r.corners.iter().copied().sum::<Vec2>() * 0.25;
    canvas_to_screen(layout.space(r.canvas), r.scale_factor, centre, view)
}

/// `{ {id, name, state, interactable, selected, rect = {x, y, width, height}}, … }`
/// for every visible Selectable, in draw order; `rect` in screen pixels (absent for
/// a world-canvas Selectable behind the camera).
fn list(
    lua: &Lua,
    world: &World,
    (layout, view): (&UiLayout, &UiView),
    events: &EventSystem,
) -> mlua::Result<Table> {
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
        if let Some((lo, hi)) = rect_screen_bounds(layout.space(rect.canvas), rect, view) {
            let r = lua.create_table()?;
            r.set("x", lo.x)?;
            r.set("y", lo.y)?;
            r.set("width", hi.x - lo.x)?;
            r.set("height", hi.y - lo.y)?;
            e.set("rect", r)?;
        }
        out.push(e)?;
    }
    Ok(out)
}
