//! Two halves that name each other, kept in one crate: [`scripting`] owns the VM
//! (the sandbox, the script lifecycle, timers and the console log) and [`api`] owns
//! the one API surface every script, the console REPL and bot-players call. The
//! runtime installs the surface on each VM it starts, and the surface reads runtime
//! state (the console, the timer scheduler, the live instance index), so neither sits
//! below the other.

#![deny(clippy::unwrap_used)]

pub mod api;
pub mod scripting;

#[cfg(test)]
mod test_temp;
