//! The integration binary holds no provider key either (#974), under any runner:
//! keys are read only by a binary that opted in at startup
//! (`dev::providers::read_keys`), and no test binary does, so this holds under plain
//! `cargo test` as under nextest; a binary this suite spawns is told to read none
//! (`spawn`). Gated on `dev`.

use rusty::dev::providers::{environment, keys_blanked, resolve, Provider, NO_KEYS_VAR};

#[test]
fn the_integration_binary_never_opted_in() {
    assert!(keys_blanked());
}

#[test]
fn no_provider_key_resolves_in_the_integration_binary() {
    let env = environment();
    for provider in Provider::ALL {
        assert!(resolve(provider, &env).is_err(), "{}", provider.variable());
    }
}

#[test]
fn a_spawned_engine_binary_is_told_to_read_no_key() {
    let command = crate::spawn::engine_bin!("session");
    let set = command
        .get_envs()
        .any(|(k, v)| k == NO_KEYS_VAR && v.is_some());
    assert!(set, "a spawned engine binary would read provider keys");
}
