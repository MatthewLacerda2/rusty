//! `Editor.SetGameViewSize` and its siblings (#2207): the Game view's size dropdown
//! from Lua, Unity's Game view resolution menu (*Free Aspect*, an aspect, a fixed
//! resolution, *Low Resolution Aspect Ratios*). Through `Application`'s mailbox, as
//! `Editor.SetGizmo` is (`core::application::game_view`): a set reads back on the
//! next line and the editor window takes it at its frame boundary. Only the editor
//! window has a Game view to size, so any other host refuses a set with a warning.

use mlua::Lua;

use rusty_core::application::{Application, GameViewSetting, GameViewSize, MAX_GAME_VIEW_SIDE};
use rusty_script::api::{put, ApiCell, Reg};
use rusty_script::scripting::ConsoleLogs;

/// Add the Game view verbs to the `Editor` table.
pub(super) fn verbs(
    table: &mlua::Table,
    lua: &Lua,
    app: ApiCell<Application>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    // Set the whole setting `edit` makes of the current one, or refuse with a warning.
    let set = move |lua: &Lua, verb: &str, edit: &dyn Fn(&mut GameViewSetting)| {
        let app = app.get(lua)?;
        let mut app = app.borrow_mut();
        if !app.game_view_host() {
            console.get(lua)?.borrow_mut().warn(format!(
                "Editor.{verb}: refused: only the editor window has a Game view to size"
            ));
            return Ok(false);
        }
        let mut setting = app.game_view_setting();
        edit(&mut setting);
        app.request_game_view(setting);
        Ok(true)
    };
    // A size from two numbers, or a warning naming the bounds.
    let sized = move |lua: &Lua,
                      verb: &str,
                      (w, h): (f64, f64),
                      make: fn(u32, u32) -> GameViewSize| {
        let side = |v: f64| {
            (v.fract() == 0.0 && v >= 1.0 && v <= f64::from(MAX_GAME_VIEW_SIDE)).then_some(v as u32)
        };
        let size = side(w).zip(side(h)).map(|(w, h)| make(w, h));
        if size.is_none() {
            console.get(lua)?.borrow_mut().warn(format!(
                "Editor.{verb}: wants two whole numbers from 1 to {MAX_GAME_VIEW_SIDE}, got {w}, {h}"
            ));
        }
        Ok::<_, mlua::Error>(size)
    };
    put(
        table,
        "SetGameViewSize",
        lua.create_function(move |lua, args: (f64, f64)| {
            let Some(size) = sized(lua, "SetGameViewSize", args, GameViewSize::Fixed)? else {
                return Ok(false);
            };
            set(lua, "SetGameViewSize", &|s| s.size = size)
        }),
    )?;
    put(
        table,
        "SetGameViewAspect",
        lua.create_function(move |lua, args: (f64, f64)| {
            let Some(size) = sized(lua, "SetGameViewAspect", args, GameViewSize::Aspect)? else {
                return Ok(false);
            };
            set(lua, "SetGameViewAspect", &|s| s.size = size)
        }),
    )?;
    put(
        table,
        "SetGameViewLowResolution",
        lua.create_function(move |lua, on: bool| {
            set(lua, "SetGameViewLowResolution", &|s| s.low_resolution = on)
        }),
    )?;
    put(
        table,
        "ResetGameViewSize",
        lua.create_function(move |lua, ()| {
            set(lua, "ResetGameViewSize", &|s| {
                *s = GameViewSetting::default()
            })
        }),
    )?;
    put(
        table,
        "GetGameViewSize",
        lua.create_function(move |lua, ()| {
            let setting = app.get(lua)?.borrow().game_view_setting();
            describe(lua, setting)
        }),
    )
}

/// `{ mode, width, height, low_resolution }`: `mode` is `"Free"`, `"Aspect"` or
/// `"Fixed"`; `width` and `height` are the aspect's or the size's, absent for `Free`.
fn describe(lua: &Lua, setting: GameViewSetting) -> mlua::Result<mlua::Table> {
    let table = lua.create_table()?;
    let (mode, sides) = match setting.size {
        GameViewSize::Free => ("Free", None),
        GameViewSize::Aspect(w, h) => ("Aspect", Some((w, h))),
        GameViewSize::Fixed(w, h) => ("Fixed", Some((w, h))),
    };
    table.set("mode", mode)?;
    if let Some((w, h)) = sides {
        table.set("width", w)?;
        table.set("height", h)?;
    }
    table.set("low_resolution", setting.low_resolution)?;
    Ok(table)
}
