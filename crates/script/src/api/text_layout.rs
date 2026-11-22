use glam::Vec2;
use mlua::Table;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_core::settings::video::VideoSettings;
use rusty_scene::Scene;
use rusty_ui::{layout, text as ui_text, ScreenSize};

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
