//! The scripting layer: the Lua runtime (`ScriptManager`) that drives gameplay
//! scripts and the REPL, plus the shared console log buffer (`ConsoleLogs`).
//!
//! Split into focused submodules — `console` (log buffer + levels), `manager`
//! (runtime init / script load / live-state) and `lifecycle` (the
//! start/update/trigger/eval callbacks + REPL value rendering) — and re-exported
//! here so the public surface stays a flat `crate::scripting::*`.

mod console;
mod lifecycle;
mod manager;

pub use console::{ConsoleLogs, LogLevel};
pub use manager::ScriptManager;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_physics;
