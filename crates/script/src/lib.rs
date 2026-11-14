//! crates/script/src/lib.rs — the Lua side of the engine: the runtime and the surface it serves.
//!
//! Two halves that name each other, kept in one crate: [`scripting`] owns the VM
//! (the sandbox, the script lifecycle, timers and the console log) and [`api`] owns
//! the one API surface every script, the console REPL and bot-players call. The
//! runtime installs the surface on each VM it starts, and the surface reads runtime
//! state (the console, the timer scheduler, the live instance index), so neither sits
//! below the other.

// Panic-free sim (#195): bare `.unwrap()` is denied here (use `?` or a documented
// `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod api;
pub mod scripting;

#[cfg(test)]
mod test_temp;
