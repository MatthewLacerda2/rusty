//! src/api/text_layout.rs — the `Text` namespace's layout reads (#419, #422).
//!
//! `GetPreferredSize` (what the text wants, for layout), `GetLayout` (what it drew:
//! size, lines, the auto-sized font size, whether it was cut) and `MeasureString`
//! (an arbitrary string's unwrapped size in the entity's font and size — where an
//! input field puts its caret). Split from `text.rs` to keep it under the size cap.

use std::cell::RefCell;

use glam::Vec2;
use mlua::Table;

use super::{put, Reg};
use crate::core::video::VideoSettings;
use crate::scene::Scene;
use crate::ui::{layout, text as ui_text, ScreenSize};

type Scoped<'s> = &'s RefCell<Scene>;

/// `GetPreferredSize(id) -> w, h` (wrapped at the element's current rect width) and
/// `GetLayout(id) -> { width, height, lines, font_size, truncated }` (what draws).
pub(super) fn register<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: Scoped<'scope>,
    screen: &'scope RefCell<ScreenSize>,
    video: &'scope RefCell<VideoSettings>,
) -> Reg {
    let rect_size = move |id: u32| {
        let px = screen.borrow().pixels(&video.borrow());
        layout::rect_of(&scene.borrow().world, id, px).map(|r| r.rect.1)
    };
    put(
        table,
        "GetPreferredSize",
        scope.create_function(move |_, id: u32| {
            let width = rect_size(id).map_or(f32::INFINITY, |s| s.x);
            let text = scene.borrow().world.text(id).map(|t| t.clone());
            let size = text.map_or(Vec2::ZERO, |t| ui_text::preferred_size(&t, width));
            Ok((size.x, size.y))
        }),
    )?;
    put(
        table,
        "GetLayout",
        scope.create_function(move |lua, id: u32| {
            let text = scene.borrow().world.text(id).map(|t| t.clone());
            let (Some(text), Some(rect)) = (text, rect_size(id)) else {
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
        scope.create_function(move |_, (id, s): (u32, String)| {
            let text = scene.borrow().world.text(id).map(|t| t.clone());
            let size = text.map_or(Vec2::ZERO, |t| ui_text::measure_string(&t, &s));
            Ok((size.x, size.y))
        }),
    )
}
