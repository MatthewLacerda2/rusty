//! The three verbs over a mocked catalogue: the cache, the filter, a withdrawn
//! voice and Play. No key, no network.

use std::cell::Cell;

use scorsese_providers::api::elevenlabs::voices::Filters;
use scorsese_providers::voices::Page;

use super::super::super::voices::*;
use super::{captured, scratch};

/// The captured voices, counting how often the listing is asked for.
struct Mock {
    voices: Vec<Voice>,
    listed: Cell<usize>,
}

fn mock() -> Mock {
    let voices = captured().voices.iter().map(flatten).collect();
    Mock {
        voices,
        listed: Cell::new(0),
    }
}

impl Catalogue for Mock {
    fn builtin(&self) -> Result<Vec<Voice>, VoiceError> {
        self.listed.set(self.listed.get() + 1);
        Ok(self.voices.clone())
    }
    fn library(&self, _: &Filters) -> Result<Page, VoiceError> {
        unreachable!("rusty lists the account, not the Voice Library")
    }
    fn one(&self, id: &str) -> Result<Availability, VoiceError> {
        Ok(match self.voices.iter().find(|v| v.id == id) {
            Some(voice) => Availability::Available(voice.clone()),
            None => Availability::Unusable(Unusable::Gone),
        })
    }
    fn name(&self) -> &'static str {
        "mock"
    }
}

#[test]
fn a_cold_cache_fetches_and_a_warm_one_answers_without_asking() {
    let root = scratch("voices_cache");
    let catalogue = mock();
    let cold = list(&root, false, &catalogue, false).unwrap();
    assert_eq!(
        (cold.freshness.clone(), cold.voices.len()),
        (Freshness::Fetched, 3)
    );
    assert!(
        root.join("cache/voices").is_dir(),
        "kept under the project's cache/"
    );

    let warm = list(&root, false, &catalogue, false).unwrap();
    assert_eq!(catalogue.listed.get(), 1, "the warm cache answered");
    assert_eq!(warm.voices, cold.voices);
    let said = warm.summary("ElevenLabs", REFRESH);
    assert!(
        said.contains("cache") && said.contains(REFRESH),
        "staleness is visible: {said}"
    );
}

#[test]
fn refresh_asks_again_even_when_the_cache_is_current() {
    let root = scratch("voices_refresh");
    let catalogue = mock();
    list(&root, false, &catalogue, false).unwrap();
    let again = list(&root, false, &catalogue, true).unwrap();
    assert_eq!(
        (catalogue.listed.get(), again.freshness),
        (2, Freshness::Fetched)
    );
}

#[test]
fn find_matches_a_name_and_a_label_ignoring_case() {
    let root = scratch("voices_find");
    let catalogue = mock();
    let by_name = find(&root, false, &catalogue, "ROGER", false).unwrap();
    assert_eq!(by_name.voices.len(), 1);
    assert!(by_name.voices[0].name.starts_with("Roger"));

    let by_label = find(&root, false, &catalogue, "american", false).unwrap();
    assert!(!by_label.voices.is_empty());
    assert!(by_label
        .voices
        .iter()
        .all(|v| v.traits.iter().any(|t| t == "american")));

    let none = find(&root, false, &catalogue, "klingon", false).unwrap();
    assert!(none.voices.is_empty());
}

#[test]
fn an_empty_query_is_refused_before_anything_is_listed() {
    let catalogue = mock();
    let result = find(&scratch("voices_empty"), false, &catalogue, "  ", false);
    assert!(matches!(result, Err(VoicesError::EmptyQuery)));
    assert_eq!(catalogue.listed.get(), 0);
}

#[test]
fn an_unknown_id_reports_withdrawn_and_substitutes_nothing() {
    let catalogue = mock();
    let known = &catalogue.voices[0].id;
    assert!(
        matches!(info(false, &catalogue, known), Ok(Availability::Available(v)) if &v.id == known)
    );

    let Ok(Availability::Unusable(why)) = info(false, &catalogue, "gone-voice") else {
        panic!("an unknown id is an answer, not an error")
    };
    let said = withdrawn("gone-voice", &why);
    assert!(said.contains("gone-voice is withdrawn"), "{said}");
    assert!(
        said.contains("2026-12-31") && said.contains("Nothing was substituted"),
        "{said}"
    );
}

#[test]
fn every_verb_is_refused_during_play_before_asking() {
    let root = scratch("voices_play");
    let catalogue = mock();
    assert!(matches!(
        list(&root, true, &catalogue, false),
        Err(VoicesError::Refused(_))
    ));
    assert!(matches!(
        find(&root, true, &catalogue, "x", false),
        Err(VoicesError::Refused(_))
    ));
    assert!(matches!(
        info(true, &catalogue, "x"),
        Err(VoicesError::Refused(_))
    ));
    assert_eq!(catalogue.listed.get(), 0);
}
