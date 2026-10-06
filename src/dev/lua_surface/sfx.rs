//! src/dev/lua_surface/sfx.rs — `Sfx.Generate(brief)` (#388).
//!
//! A spending verb, so it lives here behind `dev` beside `Speech`: a shipped game
//! has no `Sfx` table. Its own namespace rather than a verb on `Sound`, because
//! every `Sound.*` verb is free and offline, and the namespace boundary is the
//! warning that this one is not.

use std::cell::RefCell;
use std::path::Path;

use mlua::{Lua, Table};

use super::super::providers::generated::{self, Realised};
use super::super::providers::sfx::{self, Draft, ElevenLabs, SfxError};
use super::super::providers::{environment, ledger, Context};
use crate::api::{put, ApiScopedCtx, Reg};

/// `Sfx.Generate(brief)` → the generated WAV's path; or `nil, why` when the brief is
/// not finished, so a loop over a weapon's twenty sounds loses none to one typo.
/// Anything that would stop every effect (Play, no key, the budget, the vendor)
/// raises.
pub(super) fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let playing: &'scope RefCell<bool> = ctx.is_playing;
    let table = lua.create_table().map_err(|e| e.to_string())?;
    put(
        &table,
        "Generate",
        scope.create_function(move |_, brief: Table| {
            let draft = draft_of(&brief)?;
            let (environment, ledger, today) = (environment(), ledger::path(), generated::today());
            let at = Context {
                root: Path::new(""),
                playing: *playing.borrow(),
                environment: &environment,
                ledger: &ledger,
                today: &today,
            };
            match sfx::generate(draft, &at, &ElevenLabs) {
                Ok(Realised::Cached(path) | Realised::Generated(path)) => {
                    Ok((Some(path.to_string_lossy().into_owned()), None))
                }
                Err(SfxError::Incomplete(why)) => Ok((None, Some(why.to_string()))),
                Err(other) => Err(mlua::Error::external(other)),
            }
        }),
    )?;
    lua.globals().set("Sfx", table).map_err(|e| e.to_string())
}

/// The brief table's fields, each optional until [`Draft::gather`] says otherwise.
fn draft_of(brief: &Table) -> mlua::Result<Draft> {
    Ok(Draft {
        text: brief.get("text")?,
        seconds: brief.get("seconds")?,
        prompt_influence: brief.get("prompt_influence")?,
        looping: brief.get("loop")?,
    })
}
