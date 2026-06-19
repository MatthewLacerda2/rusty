//! The scripting layer: the Lua runtime (`ScriptManager`) that drives gameplay
//! scripts and the REPL, plus the shared console log buffer (`ConsoleLogs`).
//!
//! Split into focused submodules — `console` (log buffer + levels), `manager`
//! (runtime init / script load / live-state) and `lifecycle` (the
//! start/update/trigger/eval callbacks + REPL value rendering) — and re-exported
//! here so the public surface stays a flat `crate::scripting::*`.

// Panic-free sim core (#195): bare `.unwrap()` is denied here (use `?` or a
// documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

mod console;
mod discovery;
mod lifecycle;
mod manager;
mod schema;

pub use console::{ConsoleLogs, LogLevel};
pub use discovery::{monobehaviour_scripts, script_label};
pub use manager::ScriptManager;
pub use schema::{parse_fields, FieldKind, ScriptField};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_console;
#[cfg(test)]
mod tests_lifecycle;
#[cfg(test)]
mod tests_manager;
#[cfg(test)]
mod tests_physics;
