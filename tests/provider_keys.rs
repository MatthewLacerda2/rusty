//! The integration binary holds no provider key either (#974), under any runner:
//! keys are read only by a binary that opted in at startup
//! (`dev::providers::read_keys`), and no test binary does, so this holds under plain
//! `cargo test` as under nextest. Gated on `dev`.

use rusty::dev::providers::{environment, keys_blanked, resolve, Provider};

#[test]
fn the_integration_binary_is_known_to_be_a_test() {
    assert!(keys_blanked());
}

#[test]
fn no_provider_key_resolves_in_the_integration_binary() {
    let env = environment();
    for provider in [Provider::Gemini, Provider::ElevenLabs, Provider::Anthropic] {
        assert!(resolve(provider, &env).is_err(), "{}", provider.variable());
    }
}
