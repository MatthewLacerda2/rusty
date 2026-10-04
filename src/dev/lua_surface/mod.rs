//! src/dev/lua_surface/mod.rs — the dev-only part of the Lua surface (#737).
//!
//! `Debug.*` and the GPU bake verbs (`Lighting.Bake`, `Reflection.Bake`, `Probe.Bake`)
//! belong to the one API surface, but they render, so they live here in the platform
//! layer and are installed onto the surface through [`api::extend`]. The arrow runs
//! `dev → api` only: the sim surface never compiles the renderer. A script sees the
//! same namespaces it always did.
//!
//! Every dev entry point that builds a world ([`Session`](super::session::Session),
//! [`Harness`](super::harness::Harness), the windowed shell) calls [`install_api`]
//! first; it is idempotent, so the order and the count don't matter.

mod bake;
mod debug;

use std::sync::Once;

use mlua::Lua;

use crate::api::{self, ApiScopedCtx, Reg};

/// Install the dev namespaces onto the Lua surface, once per process.
pub fn install_api() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| api::extend(register));
}

/// The [`api::Extension`] itself: `Debug`, then the bake verbs on the tables `api`
/// already registered.
fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    debug::register(lua, scope, ctx)?;
    bake::register(lua, scope, ctx)
}

#[cfg(test)]
mod tests;
