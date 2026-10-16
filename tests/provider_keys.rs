//! The integration binary holds no provider key either (#974). `cfg(test)` is off for
//! the library here, so the blanking rests on nextest's `NEXTEST` variable reaching
//! this process; this fails if the library stops honouring it, before a test could
//! spend. Plain `cargo test` (the mutation jobs' runner) sets no such variable and is
//! not the gate, so there these return early. Gated on `dev`.

fn under_nextest() -> bool {
    std::env::var_os("NEXTEST").is_some()
}

use rusty::dev::providers::{environment, keys_blanked, resolve, Provider};

#[test]
fn the_integration_binary_is_known_to_be_a_test() {
    assert!(keys_blanked() || !under_nextest());
}

#[test]
fn no_provider_key_resolves_in_the_integration_binary() {
    if !under_nextest() {
        return;
    }
    let env = environment();
    for provider in [Provider::Gemini, Provider::ElevenLabs, Provider::Anthropic] {
        assert!(resolve(provider, &env).is_err(), "{}", provider.variable());
    }
}
