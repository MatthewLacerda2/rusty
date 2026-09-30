//! src/api/ui/ — `UI` namespace: EventSystem-level UI verbs (#417, #420).
//!
//! Reads the in-game UI's computed layout. `UI.GetRect(id)` returns an element's
//! rect in its canvas's reference units and in screen pixels, computed on demand
//! from the live scene (so it reflects a change made earlier in the same script, in
//! edit mode as well as in Play) with the same math as the per-tick layout system —
//! through the active camera, so markers sit where they draw (#429). The pointer
//! and focus verbs (#420) are in `events`; `view` builds the screen + camera they
//! all compute against. `UI.Create(kind, [parent])` (#422) is the Create ▸ UI
//! menu's API face: the same `create_ui` builds the widget.

mod events;
mod view;

pub use events::register_events;

use std::cell::RefCell;

use glam::Vec2;
use mlua::{Lua, Table};
use serde_json::{json, Value};

use super::{put, Reg};
use crate::core::video::VideoSettings;
use crate::scene::authoring::ui_widgets::{create_ui, UiWidget};
use crate::scene::{Camera, Scene};
use crate::ui::space::{rect_screen_bounds, space_of, CanvasSpace};
use crate::ui::{layout, ScreenSize, UiRect};

/// Register the `UI` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    (screen, video): (&'scope RefCell<ScreenSize>, &'scope RefCell<VideoSettings>),
    camera: &'scope RefCell<Camera>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    let ui_view = move || view::ui_view(screen, video, camera);
    put(
        &table,
        "GetRect",
        scope.create_function(move |lua, id: u32| {
            let view = ui_view();
            let world = &scene.borrow().world;
            let Some(rect) = layout::rect_in(world, id, &view) else {
                return Ok(None);
            };
            let space = space_of(world, rect.canvas, &view);
            let t = rect_table(lua, &rect, rect_screen_bounds(space, &rect, &view))?;
            t.set("world", matches!(space, CanvasSpace::World(_)))?;
            Ok(Some(t))
        }),
    )?;
    put(
        &table,
        "GetScreenSize",
        scope.create_function(move |_, ()| {
            let px = screen.borrow().pixels(&video.borrow());
            Ok((px.x, px.y))
        }),
    )?;
    put(
        &table,
        "Create",
        scope.create_function(|_, (kind, parent): (String, Option<u32>)| {
            let widget = UiWidget::parse(&kind).ok_or_else(|| {
                let all: Vec<_> = UiWidget::ALL.iter().map(|w| w.label()).collect();
                mlua::Error::RuntimeError(format!(
                    "UI.Create: unknown widget '{kind}' (one of: {})",
                    all.join(", ")
                ))
            })?;
            Ok(create_ui(&mut scene.borrow_mut(), widget, parent))
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

/// The Lua shape of a rect — the same keys as [`rect_value`]; `screen` (its
/// screen-pixel box) is absent for a world-canvas rect behind the camera.
fn rect_table<'lua>(
    lua: &'lua Lua,
    r: &UiRect,
    screen: Option<(Vec2, Vec2)>,
) -> mlua::Result<Table<'lua>> {
    let t = box_table(lua, r.bounds())?;
    if let Some(screen) = screen {
        t.set("screen", box_table(lua, screen)?)?;
    }
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
