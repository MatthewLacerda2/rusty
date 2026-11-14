//! The integration binary holds no provider key either (#974), under any runner:
//! keys are read only by a binary that opted in at startup
//! (`dev::providers::read_keys`), and no test binary does (`rusty_dev::no_provider_keys!`,
//! which each member crate linking `rusty-dev` expands too), so this holds under plain
//! `cargo test` as under nextest; a binary this suite spawns is told to read none
//! (`spawn`). Gated on `dev`.

use rusty::dev::providers::NO_KEYS_VAR;

// The tripwire every test binary linking `rusty-dev` expands (`key-guard`).
rusty_dev::no_provider_keys!();

#[test]
fn a_spawned_engine_binary_is_told_to_read_no_key() {
    let command = crate::spawn::engine_bin!("session");
    let set = command
        .get_envs()
        .any(|(k, v)| k == NO_KEYS_VAR && v.is_some());
    assert!(set, "a spawned engine binary would read provider keys");
}
