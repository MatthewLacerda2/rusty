//! crates/script/src/api/text_layout.rs — the `Text` namespace's layout reads (#419, #422).
//!
//! `GetPreferredSize` (what the text wants, for layout), `GetLayout` (what it drew:
//! size, lines, the auto-sized font size, whether it was cut) and `MeasureString`
//! (an arbitrary string's unwrapped size in the entity's font and size — where an
//! input field puts its caret). Split from `text.rs` to keep it under the size cap.

use glam::Vec2;
use mlua::Table;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_core::settings::video::VideoSettings;
use rusty_scene::Scene;
use rusty_ui::{layout, text as ui_text, ScreenSize};

/// `GetPreferredSize(id) -> w, h` (wrapped at the element's current rect width) and
/// `GetLayout(id) -> { width, height, lines, font_size, truncated }` (what draws).
pub(super) fn register(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    screen: ApiCell<ScreenSize>,
    video: ApiCell<VideoSettings>,
) -> Reg {
    let rect_size = move |lua: &mlua::Lua, id: u32| -> mlua::Result<_> {
        let (scene, screen, video) = (scene.get(lua)?, screen.get(lua)?, video.get(lua)?);
        let view = screen.borrow().view(&video.borrow(), None);
        let size = layout::rect_in(&scene.borrow().world, id, &view).map(|r| r.rect.1);
        Ok(size)
    };
    put(
        table,
        "GetPreferredSize",
        lua.create_function(move |lua, id: EntityArg| {
            let id = id.read();
            let width = rect_size(lua, id)?.map_or(f32::INFINITY, |s| s.x);
            let text = scene.get(lua)?.borrow().world.text(id).map(|t| t.clone());
            let size = text.map_or(Vec2::ZERO, |t| ui_text::preferred_size(&t, width));
            Ok((size.x, size.y))
        }),
    )?;
    put(
        table,
        "GetLayout",
        lua.create_function(move |lua, id: EntityArg| {
            let id = id.read();
            let text = scene.get(lua)?.borrow().world.text(id).map(|t| t.clone());
            let (Some(text), Some(rect)) = (text, rect_size(lua, id)?) else {
                return Ok(None);
            };
            let l = ui_text::layout_text(&text, rect);
            let t = lua.create_table()?;
            t.set("width", l.size.x)?;
            t.set("height", l.size.y)?;
            t.set("lines", l.lines)?;
            t.set("font_size", l.font_size)?;
            t.set("truncated", l.truncated)?;
            Ok(Some(t))
        }),
    )?;
    put(
        table,
        "MeasureString",
        lua.create_function(move |lua, (id, s): (EntityArg, String)| {
            let text = scene
                .get(lua)?
                .borrow()
                .world
                .text(id.read())
                .map(|t| t.clone());
            let size = text.map_or(Vec2::ZERO, |t| ui_text::measure_string(&t, &s));
            Ok((size.x, size.y))
        }),
    )
}
