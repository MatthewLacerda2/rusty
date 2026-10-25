//! The scripting layer: the Lua runtime (`ScriptManager`) that drives gameplay
//! scripts and the REPL, plus the shared console log buffer (`ConsoleLogs`).
//!
//! Split into focused submodules — `console` (log buffer + levels), `manager`
//! (runtime init / live-state), `loader` (script compile + the queued spawn
//! scan), `lifecycle` (the hook dispatch: init/update/trigger), `eval` (the
//! REPL evaluator + API-surface walk), `shared` (the API's cells, owned, for a
//! second VM over the same world), `timers` (the `Timer` scheduler: invokes
//! and coroutines, #444), `callbacks` (the lifecycle-callback
//! names, kept public and namespaced) and `api_doc` (the `docs/api/` reference,
//! assembled for the drift gates and MCP) — and re-exported here so the public
//! surface stays a flat `crate::scripting::*`.

// Panic-free sim core (#195): bare `.unwrap()` is denied here (use `?` or a
// documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod api_doc;
pub mod callbacks;
mod cells;
mod console;
mod discovery;
mod eval;
pub mod instances;
mod lifecycle;
mod loader;
mod manager;
mod sandbox;
mod schema;
mod shared;
mod timers;

pub use console::{ConsoleLogs, LogLevel};
pub use discovery::{monobehaviour_scripts, script_label};
pub use manager::ScriptManager;
pub(crate) use sandbox::require_sim_vm;
#[cfg(feature = "dev")]
pub(crate) use sandbox::sandboxed_vm;
pub use schema::{parse_fields, FieldKind, ScriptField};
pub use shared::ApiCells;
pub use timers::TimerScheduler;
pub(crate) use timers::{tween, wait, Clock, Target, Wait, Work};

#[cfg(test)]
mod tests;
