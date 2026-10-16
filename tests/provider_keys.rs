//! The integration binary holds no provider key either (#974). `cfg(test)` is off for
//! the library here, so the blanking rests on nextest's `NEXTEST` variable reaching
//! this process; this fails if it ever stops, before a test could spend. Gated on `dev`.

use rusty::dev::providers::{environment, keys_blanked, resolve, Provider};

#[test]
fn the_integration_binary_is_known_to_be_a_test() {
    assert!(
        keys_blanked(),
        "NEXTEST is unset: run the suite with cargo nextest"
    );
}

#[test]
fn no_provider_key_resolves_in_the_integration_binary() {
    let env = environment();
    for provider in [Provider::Gemini, Provider::ElevenLabs, Provider::Anthropic] {
        assert!(resolve(provider, &env).is_err(), "{}", provider.variable());
    }
}
