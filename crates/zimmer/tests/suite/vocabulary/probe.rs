//! What a document type takes, read back from its own parser: every type refuses an
//! unknown key or word, and serde's refusal lists the ones it would have taken.

use serde::de::DeserializeOwned;

const STRANGER: &str = "__not_a_key__";

/// The keys `T` takes, from the refusal of an object holding `body` and one stranger.
/// `body` is the tag of an internally tagged enum (`"kind": "karplus",`), or empty.
pub(super) fn keys<T: DeserializeOwned>(body: &str) -> Vec<String> {
    refused::<T>(&format!("{{{body}\"{STRANGER}\": 0}}"))
}

/// The words a word-valued type (`Wave`, `Easing`) takes.
pub(super) fn words<T: DeserializeOwned>() -> Vec<String> {
    refused::<T>(&format!("\"{STRANGER}\""))
}

/// The words a tagged enum's `tag` takes (`kind` for a source, `fx` for an effect).
pub(super) fn tags<T: DeserializeOwned>(tag: &str) -> Vec<String> {
    refused::<T>(&format!("{{\"{tag}\": \"{STRANGER}\"}}"))
}

/// The keys of every variant of a tagged enum, its tag among them.
pub(super) fn tagged<T: DeserializeOwned>(tag: &str) -> Vec<String> {
    let mut taken = vec![tag.to_owned()];
    for variant in tags::<T>(tag) {
        taken.extend(keys::<T>(&format!("\"{tag}\": \"{variant}\",")));
    }
    taken
}

fn refused<T: DeserializeOwned>(json: &str) -> Vec<String> {
    let error = match serde_json::from_str::<T>(json) {
        Ok(_) => panic!("{json} parsed, so its refusal cannot list what the type takes"),
        Err(error) => error.to_string(),
    };
    let Some((_, expected)) = error.split_once("expected") else {
        panic!("{json} was refused without a list of what it takes: {error}");
    };
    let listed: Vec<String> = expected
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();
    assert!(!listed.is_empty(), "{json}: nothing listed in `{error}`");
    listed
}

#[test]
fn a_refusal_lists_every_key_and_word() {
    use rusty_zimmer::patch::{Adsr, Source, Wave};
    assert_eq!(keys::<Adsr>(""), ["a", "d", "s", "r", "curve"]);
    assert_eq!(
        keys::<Source>("\"kind\": \"karplus\","),
        ["damping", "brightness"]
    );
    assert_eq!(words::<Wave>(), ["sine", "triangle", "saw", "square"]);
    assert_eq!(tags::<Source>("kind").len(), 6);
}
