//! The voice listing (#387) read against a real ElevenLabs response: scorsese's
//! capture of `/v1/voices?category=premade` (2026-08-05, cut to three voices,
//! account ids scrubbed), copied to `tests/fixtures/elevenlabs/`. `/v2/voices`
//! sends the same voice objects inside a paged envelope (`has_more`,
//! `next_page_token`), so the pages here are that body split in two.

mod catalogue;

use std::cell::RefCell;

use scorsese_providers::api::http::HttpError;

use super::super::voices::*;
use super::*;

const PREMADE: &str = include_str!("../../../../../tests/fixtures/elevenlabs/premade.json");

fn captured() -> AccountPage {
    serde_json::from_str(PREMADE).expect("the captured body parses as a page")
}

/// The captured voices as two pages: two voices then one, joined by a token.
fn two_pages(token: Option<&str>) -> Result<AccountPage, HttpError> {
    let mut page = captured();
    match token {
        None => {
            page.voices.truncate(2);
            page.has_more = true;
            page.next_page_token = Some("after two/=".into());
        }
        Some(_) => page.voices = page.voices.split_off(2),
    }
    Ok(page)
}

#[test]
fn every_page_is_read_and_assembled_in_order() {
    let asked = RefCell::new(Vec::new());
    let voices = every_page(|token| {
        asked.borrow_mut().push(token.map(str::to_owned));
        two_pages(token)
    })
    .unwrap();
    let ids: Vec<&str> = voices.iter().map(|v| v.id.as_str()).collect();
    let whole: Vec<String> = captured().voices.into_iter().map(|v| v.voice_id).collect();
    assert_eq!(ids, whole, "all three voices, first page first");
    assert_eq!(*asked.borrow(), [None, Some("after two/=".to_owned())]);
}

#[test]
fn more_promised_with_no_token_is_an_error_not_a_short_list() {
    let result = every_page(|_| {
        let mut page = captured();
        page.has_more = true;
        Ok(page)
    });
    let said = result.unwrap_err().to_string();
    assert!(said.contains("stopped short"), "{said}");
}

#[test]
fn a_vendor_that_never_stops_paging_is_cut_off() {
    let result = every_page(|_| {
        Ok(AccountPage {
            has_more: true,
            next_page_token: Some("again".into()),
            ..AccountPage::default()
        })
    });
    assert!(result.unwrap_err().to_string().contains("still paging"));
}

#[test]
fn the_page_token_is_escaped_into_the_url() {
    assert_eq!(
        page_url(Some("a b/=")),
        "https://api.elevenlabs.io/v2/voices?page_size=100&next_page_token=a%20b%2F%3D"
    );
    assert!(page_url(None).ends_with("?page_size=100"));
}

#[test]
fn a_captured_voice_keeps_every_label_it_was_sent() {
    let page = captured();
    for found in &page.voices {
        let voice = flatten(found);
        assert_eq!(voice.id, found.voice_id);
        for label in found.labels.values() {
            assert!(voice.traits.contains(label), "{} lost {label}", voice.name);
        }
    }
    let roger = flatten(&page.voices[0]);
    assert!(roger.name.starts_with("Roger"));
    assert!(roger.traits.iter().any(|t| t == "american"));
    assert!(roger.preview.is_some());
}

#[test]
fn no_key_and_a_cold_cache_says_where_it_looked_and_calls_nothing() {
    let empty = Environment::of(Vec::<(String, String)>::new());
    let said = Account {
        environment: &empty,
    }
    .builtin()
    .unwrap_err()
    .to_string();
    assert!(said.contains("ELEVENLABS_API_KEY"), "{said}");
}
