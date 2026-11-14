//! A test never holds a provider key (#974): the tripwire that keeps the live
//! surface, which calls the real vendor, from spending under test.

use super::super::credentials::{blanked, is_key, without_keys, NO_KEYS_VAR};
use super::*;

#[test]
fn a_test_binary_never_opted_in() {
    // Holds under plain `cargo test` too: no test binary calls `read_keys`, and
    // neither `cfg(test)` nor the runner is what decides it.
    assert!(keys_blanked());
}

#[test]
fn keys_are_read_only_by_a_process_that_opted_in_and_was_not_told_not_to() {
    // Must flag: no opt-in, or started with the variable set (a spawned binary).
    assert!(blanked(false, false));
    assert!(blanked(false, true));
    assert!(blanked(true, true));
    // Must pass: an authoring binary run by hand.
    assert!(!blanked(true, false));
}

#[test]
fn a_blanked_environment_drops_every_key_and_says_why() {
    // Must flag: the key is set, yet nothing resolves, and the refusal says this
    // process reads no keys rather than that none was set.
    for provider in Provider::ALL {
        let vars = [(provider.variable().to_owned(), KEY.to_owned())];
        let refusal = resolve(provider, &without_keys(vars)).unwrap_err();
        assert!(
            matches!(refusal, Refusal::KeysNotRead { .. }),
            "{refusal:?}"
        );
        assert!(refusal.to_string().contains("session-mcp"), "{refusal}");
    }
}

#[test]
fn a_process_that_reads_keys_is_told_where_it_looked() {
    // Must pass: without the marker a miss is the ordinary "set the variable" one.
    let refusal = resolve(Provider::Gemini, &Environment::default()).unwrap_err();
    assert!(matches!(refusal, Refusal::MissingKey { .. }), "{refusal:?}");
}

#[test]
fn everything_but_the_keys_is_kept_and_no_dotenv_is_read() {
    // Must pass: what is not a key reaches the caller untouched.
    let env = without_keys([("PATH".to_owned(), "/bin".to_owned())]);
    assert_eq!(env.get("PATH"), Some("/bin"));
    assert_eq!(env.get(NO_KEYS_VAR), Some("1"));
    assert!(env.dotenv().is_none());
    assert!(environment().dotenv().is_none(), "a test read a .env");
}

#[test]
fn every_provider_variable_is_one_the_blanking_drops() {
    for provider in Provider::ALL {
        assert!(is_key(provider.variable()), "{}", provider.variable());
    }
    assert!(!is_key("PATH"));
}

#[test]
fn the_live_environment_holds_no_key_under_test() {
    let env = environment();
    for provider in Provider::ALL {
        assert!(
            env.get(provider.variable()).is_none(),
            "{}",
            provider.variable()
        );
    }
}
