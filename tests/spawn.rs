//! The one way a test starts an engine binary (#974): [`engine_bin!`] names it by
//! its Cargo target and hands back a `Command` that holds no provider key.
//!
//! `session` and `session-mcp` opt in to provider keys at startup, so started
//! bare under a runner that sets nothing, on a machine with a key, they would hold
//! it. Every spawn goes through here, which sets `RUSTY_NO_PROVIDER_KEYS`, and the
//! `engine-spawn` lint fails a `CARGO_BIN_EXE_` named anywhere else, so a spawned
//! binary holds no key under any runner. Gated on `dev`, like every test that
//! spawns one.

use std::process::Command;

use rusty::dev::providers::NO_KEYS_VAR;

/// A `Command` for the engine binary at `exe`, told to read no provider key.
pub fn command(exe: &str) -> Command {
    let mut command = Command::new(exe);
    command.env(NO_KEYS_VAR, "1");
    command
}

/// `engine_bin!("session")`: a [`command`] for that binary of this package.
macro_rules! engine_bin {
    ($name:literal) => {
        $crate::spawn::command(env!(concat!("CARGO_BIN_EXE_", $name)))
    };
}
pub(crate) use engine_bin;
