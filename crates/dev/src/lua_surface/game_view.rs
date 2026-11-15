//! `Editor.SetGameViewSize` and its siblings (#2207): the Game view's size dropdown
//! from Lua, Unity's Game view resolution menu (*Free Aspect*, an aspect, a fixed
//! resolution, *Low Resolution Aspect Ratios*). Through `Application`'s mailbox, as
//! `Editor.SetGizmo` is (`core::application::game_view`): a set reads back on the
//! next line and the editor window takes it at its frame boundary. Only the editor
//! window has a Game view to size, so any other host refuses a set with a warning.

use mlua::{FromLuaMulti, Lua};

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
    let cells = (app, console);
    let fixed = |s, wh| sized(wh, GameViewSize::Fixed).map(|size| GameViewSetting { size, ..s });
    put(
        table,
        "SetGameViewSize",
        setter(lua, cells, "SetGameViewSize", fixed),
    )?;
    let aspect = |s, wh| sized(wh, GameViewSize::Aspect).map(|size| GameViewSetting { size, ..s });
    put(
        table,
        "SetGameViewAspect",
        setter(lua, cells, "SetGameViewAspect", aspect),
    )?;
    let low = |s, on| {
        Ok(GameViewSetting {
            low_resolution: on,
            ..s
        })
    };
    put(
        table,
        "SetGameViewLowResolution",
        setter(lua, cells, "SetGameViewLowResolution", low),
    )?;
    let reset = |_, ()| Ok(GameViewSetting::default());
    put(
        table,
        "ResetGameViewSize",
        setter(lua, cells, "ResetGameViewSize", reset),
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

/// The whole setting a verb makes of the current one and its arguments, or why
/// the arguments are refused.
type Edit<A> = fn(GameViewSetting, A) -> Result<GameViewSetting, String>;

/// `Editor.<verb>`: `true` once it asked for the setting `edit` makes; `false`, with
/// a warning, for refused arguments or a host with no Game view to size.
fn setter<A: FromLuaMulti + 'static>(
    lua: &Lua,
    (app, console): (ApiCell<Application>, ApiCell<ConsoleLogs>),
    verb: &'static str,
    edit: Edit<A>,
) -> mlua::Result<mlua::Function> {
    lua.create_function(move |lua, args: A| {
        let app = app.get(lua)?;
        let mut app = app.borrow_mut();
        let edited = match app.game_view_host() {
            true => edit(app.game_view_setting(), args),
            false => Err("refused: only the editor window has a Game view to size".to_string()),
        };
        match edited {
            Ok(setting) => app.request_game_view(setting),
            Err(why) => {
                let warning = format!("Editor.{verb}: {why}");
                console.get(lua)?.borrow_mut().warn(warning);
                return Ok(false);
            }
        }
        Ok(true)
    })
}

/// A size from two numbers: whole, from 1 to [`MAX_GAME_VIEW_SIDE`].
fn sized((w, h): (f64, f64), make: fn(u32, u32) -> GameViewSize) -> Result<GameViewSize, String> {
    let side = |v: f64| {
        (v.fract() == 0.0 && (1.0..=f64::from(MAX_GAME_VIEW_SIDE)).contains(&v)).then_some(v as u32)
    };
    let size = side(w).zip(side(h)).map(|(w, h)| make(w, h));
    size.ok_or(format!(
        "wants two whole numbers from 1 to {MAX_GAME_VIEW_SIDE}, got {w}, {h}"
    ))
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
