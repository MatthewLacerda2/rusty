//! Points are UI screen pixels: bottom-left origin, y-up, the frame of `UI.GetRect(id).screen`.

use glam::Vec2;
use mlua::{Lua, Table};

use super::super::{global_table, put, ApiCell, ApiCtx, Reg};
use super::view::{pointer, ui_view};
use crate::api::entity_arg::EntityArg;
use rusty_ecs::World;
use rusty_ui::events::{find_selectable, is_interactable, is_under, raycast_pointer};
use rusty_ui::space::{canvas_to_screen, rect_screen_bounds};
use rusty_ui::{EventSystem, UiLayout, UiRect, UiView};

pub fn register_events(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let table = &global_table(lua, "UI")?;
    let (scene, input, events) = (ctx.scene, ctx.input, ctx.event_system);
    let (screen, video, camera, physics) = (ctx.screen, ctx.video, ctx.camera, ctx.physics);
    let view = move |lua: &Lua| -> mlua::Result<UiView> {
        let (screen, video, camera) = (screen.get(lua)?, video.get(lua)?, camera.get(lua)?);
        let scene = scene.get(lua)?;
        let view = ui_view(&screen, &video, &camera, &scene.borrow());
        Ok(view)
    };
    register_selection(lua, table, events)?;
    let f = lua.create_function(move |lua, (x, y): (f32, f32)| {
        let scene = &*scene.get(lua)?;
        let (view, world) = (view(lua)?, &scene.borrow().world);
        let layout = UiLayout::compute_in(world, &view);
        let at = pointer(&view, &*physics.get(lua)?, Some(Vec2::new(x, y)));
        Ok(raycast_pointer(world, &layout, &at))
    });
    put(table, "Raycast", f)?;
    register_find(lua, table, ctx)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        let input = &*input.get(lua)?;
        let id = id.live("UI", "Click")?;
        let scene = &*scene.get(lua)?;
        let (view, world) = (view(lua)?, &scene.borrow().world);
        let layout = UiLayout::compute_in(world, &view);
        let Some(centre) = layout.get(id).and_then(|r| centre_px(&layout, r, &view)) else {
            return Ok(false);
        };
        let mut input = input.borrow_mut();
        // A locked cursor clicks through the screen centre (world canvases only).
        let locked = input.cursor().locked;
        let at = pointer(&view, &*physics.get(lua)?, (!locked).then_some(centre));
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
    let f = lua.create_function(move |lua, ()| {
        scene.with(lua, |scene| {
            let (view, world) = (view(lua)?, &scene.borrow().world);
            let layout = UiLayout::compute_in(world, &view);
            list(lua, world, (&layout, &view), &events.get(lua)?.borrow())
        })
    });
    put(table, "List", f)
}

fn register_selection(lua: &Lua, table: &Table, events: ApiCell<EventSystem>) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        events.get(lua)?.borrow_mut().set_selected(id.get());
        Ok(())
    });
    put(table, "SetSelected", f)?;
    let f = lua.create_function(move |lua, ()| Ok(events.get(lua)?.borrow().selected()));
    put(table, "GetSelected", f)?;
    let f = lua.create_function(move |lua, ()| Ok(events.get(lua)?.borrow().is_pointer_over_ui()));
    put(table, "IsPointerOverUI", f)?;
    let f = lua.create_function(move |lua, ()| Ok(events.get(lua)?.borrow().is_pointer_consumed()));
    put(table, "IsPointerConsumed", f)
}

fn register_find(lua: &mlua::Lua, table: &Table, ctx: &ApiCtx) -> Reg {
    let (scene, screen, video, camera) = (ctx.scene, ctx.screen, ctx.video, ctx.camera);
    let f = lua.create_function(move |lua, (id, dir): (EntityArg, String)| {
        let scene = &*scene.get(lua)?;
        let Some(dir) = direction(&dir) else {
            return Err(mlua::Error::RuntimeError(format!(
                "UI.FindSelectable: unknown direction '{dir}' (Up, Down, Left, Right)"
            )));
        };
        let scene = &scene.borrow();
        let layout = UiLayout::compute_in(
            &scene.world,
            &ui_view(
                &*screen.get(lua)?,
                &*video.get(lua)?,
                &*camera.get(lua)?,
                scene,
            ),
        );
        let world = &scene.world;
        Ok(find_selectable(world, &layout, id.read(), dir))
    });
    put(table, "FindSelectable", f)
}

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
        if !world.has_selectable(id) || !world.is_active_in_hierarchy(id) {
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
