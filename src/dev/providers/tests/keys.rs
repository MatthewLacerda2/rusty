//! A test never holds a provider key (#974): the tripwire that keeps the live
//! surface, which calls the real vendor, from spending under test.

use super::super::credentials::{is_key, without_keys};
use super::*;

/// Every provider rusty can name, spelled as a `match` so a new one fails to
/// compile here until it is added.
fn every_provider() -> [Provider; 3] {
    let all = [Provider::Gemini, Provider::ElevenLabs, Provider::Anthropic];
    for provider in all {
        match provider {
            Provider::Gemini | Provider::ElevenLabs | Provider::Anthropic => {}
        }
    }
    all
}

#[test]
fn this_process_is_known_to_be_a_test() {
    assert!(keys_blanked());
}

#[test]
fn a_key_in_the_process_environment_is_dropped_under_test() {
    // Must flag: the key is set, yet nothing resolves.
    for provider in every_provider() {
        let vars = [(provider.variable().to_owned(), KEY.to_owned())];
        let refusal = resolve(provider, &without_keys(vars)).unwrap_err();
        assert!(matches!(refusal, Refusal::MissingKey { .. }), "{refusal:?}");
    }
}

#[test]
fn everything_but_the_keys_is_kept_and_no_dotenv_is_read() {
    // Must pass: what is not a key reaches the caller untouched.
    let env = without_keys([("PATH".to_owned(), "/bin".to_owned())]);
    assert_eq!(env.get("PATH"), Some("/bin"));
    assert!(env.dotenv().is_none());
    assert!(environment().dotenv().is_none(), "a test read a .env");
}

#[test]
fn every_provider_variable_is_one_the_blanking_drops() {
    for provider in every_provider() {
        assert!(is_key(provider.variable()), "{}", provider.variable());
    }
    assert!(!is_key("PATH"));
}

#[test]
fn the_live_environment_holds_no_key_under_test() {
    let env = environment();
    for provider in every_provider() {
        assert!(
            env.get(provider.variable()).is_none(),
            "{}",
            provider.variable()
        );
    }
}
