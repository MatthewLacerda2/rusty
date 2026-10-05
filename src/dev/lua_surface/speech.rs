//! src/dev/lua_surface/speech.rs — `Speech.Generate(brief)` (#386).
//!
//! The one spending verb on the Lua surface, so the one that lives here, behind
//! `dev`, beside the bakes: a shipped game has no `Speech` table at all. Reachable
//! over MCP for free, since the bridge is one `eval` over this surface.

use std::cell::RefCell;
use std::path::Path;

use mlua::{Lua, Table};

use super::super::providers::generated::{self, Realised};
use super::super::providers::speech::{self, Context, Draft, ElevenLabs, SpeechError};
use super::super::providers::{environment, ledger};
use crate::api::{put, ApiScopedCtx, Reg};

/// `Speech.Generate(brief)` → the generated WAV's path; or `nil, why` when the brief
/// is not finished, so a loop over twenty lines loses none to one typo. Anything
/// that would stop every line (Play, no key, the budget, the vendor) raises.
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
            match speech::generate(draft, &at, &ElevenLabs) {
                Ok(Realised::Cached(path) | Realised::Generated(path)) => {
                    Ok((Some(path.to_string_lossy().into_owned()), None))
                }
                Err(SpeechError::Incomplete(why)) => Ok((None, Some(why.to_string()))),
                Err(other) => Err(mlua::Error::external(other)),
            }
        }),
    )?;
    lua.globals()
        .set("Speech", table)
        .map_err(|e| e.to_string())
}

/// The brief table's fields, each optional until [`Draft::gather`] says otherwise.
fn draft_of(brief: &Table) -> mlua::Result<Draft> {
    Ok(Draft {
        text: brief.get("text")?,
        voice: brief.get("voice")?,
        model: brief.get("model")?,
        language: brief.get("language")?,
        seed: brief.get("seed")?,
    })
}
