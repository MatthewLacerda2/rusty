//! src/dev/lua_surface/mod.rs — the dev-only part of the Lua surface (#737).
//!
//! `Debug.*`, the GPU bake verbs (`Lighting.Bake`, `Reflection.Bake`, `Probe.Bake`,
//! `Lighting.Generate`) and the provider verbs `Speech.Generate` (#386) and
//! `Sfx.Generate` (#388), the play switch `Editor.*` (#1360) and the audio capture
//! `Audio.StartCapture` (#1579) belong to the one API surface, but they render, spend or
//! record, so
//! they live here in the platform layer and are installed onto the surface through
//! [`api::extend`]. The arrow runs
//! `dev → api` only: the sim surface never compiles the renderer. A script sees the
//! same namespaces it always did.
//!
//! Every dev entry point that builds a world ([`Session`](super::session::Session),
//! [`Harness`](super::harness::Harness), the windowed shell) calls [`install_api`]
//! first; it is idempotent, so the order and the count don't matter.

// sorted: one module per dev namespace; kept in order so parallel branches don't collide
mod bake;
mod capture;
mod debug;
mod editor;
mod generate;
mod sfx;
mod speech;
mod voices;

use std::sync::Once;

use mlua::Lua;

use crate::api::{self, ApiScopedCtx, Reg};

/// Install the dev namespaces onto the Lua surface, once per process.
pub fn install_api() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| api::extend(register));
}

/// The [`api::Extension`] itself: `Debug`, the bake verbs and the audio capture
/// (#1579) on the tables `api` already registered, `Editor` (#1360), `Sfx` (#388), and `Speech` (#386) with its voice verbs (#387).
fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    // sorted: each registrar adds its own table or extends one `api` made; `voices`
    // extends `Speech`, which `speech` sorts before
    bake::register(lua, scope, ctx)?;
    capture::register(lua, scope, ctx)?;
    debug::register(lua, scope, ctx)?;
    editor::register(lua, scope, ctx)?;
    generate::register(lua, scope, ctx)?;
    sfx::register(lua, scope, ctx)?;
    speech::register(lua, scope, ctx)?;
    voices::register(lua, scope, ctx)
}

#[cfg(test)]
mod editor_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod undo_tests;
#[cfg(test)]
mod voices_tests;
