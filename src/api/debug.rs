//! src/api/debug.rs — Debug API (DEV-ONLY)
//!
//! `Debug.Log/Warn/Error`. Registered only in dev builds — stripped from the
//! shipped game, like Unity `Debug.*` under `[Conditional]`.
//!
//! Allowed deps: dev::console.
//! Status: IMPLEMENTED (dev-only) — gated bindings in
//! `scripting::bindings::world::register_debug`, called under
//! `#[cfg(feature = "dev")]` (issue #5).
