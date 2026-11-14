//! The root's own unit tests hold no provider key (#974), under plain `cargo test`
//! as under nextest: `cfg(test)` is per crate and the dev layer is a crate of its
//! own, so what blanks the keys here is that no test binary opts in
//! (`dev::providers::read_keys`).

use crate::dev::providers::{environment, keys_blanked, resolve, Provider};

#[test]
fn the_root_unit_tests_hold_no_provider_key() {
    assert!(keys_blanked());
    let env = environment();
    for provider in [
        Provider::Gemini,
        Provider::ElevenLabs,
        Provider::Anthropic,
        Provider::Pixabay,
    ] {
        assert!(resolve(provider, &env).is_err(), "{}", provider.variable());
    }
}
