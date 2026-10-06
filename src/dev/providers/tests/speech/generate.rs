//! One line through the boundary (#386), with a mock speaker that answers with a
//! committed MP3 fixture and counts its calls. No test holds the real client.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

use scorsese_providers::api::elevenlabs::speech::Speak;

use super::super::super::generated::{Realised, Sidecar};
use super::super::super::speech::*;
use super::super::super::{Environment, Ledger, Refusal, Secret};
use super::super::{keyed, scratch};
use super::draft;

const MP3: &[u8] = include_bytes!("../../../../asset/audio/fixtures/loop.mp3");

#[derive(Default)]
struct Mock {
    calls: Cell<u32>,
    last: RefCell<Option<(String, String, Speak)>>,
    refuse: Option<String>,
}

impl Speaker for Mock {
    fn speak(&self, key: &Secret, voice: &str, body: &Speak) -> Result<Vec<u8>, String> {
        self.calls.set(self.calls.get() + 1);
        *self.last.borrow_mut() = Some((key.expose().into(), voice.into(), body.clone()));
        self.refuse.clone().map_or(Ok(MP3.to_vec()), Err)
    }
}

fn run(
    root: &Path,
    playing: bool,
    env: &Environment,
    mock: &Mock,
) -> Result<Realised, SpeechError> {
    let ledger = root.join("provider_budget.json");
    let at = Context {
        root,
        playing,
        environment: env,
        ledger: &ledger,
        today: "2026-10-05",
    };
    generate(draft("Contact, second floor."), &at, mock)
}

fn path(r: Realised) -> PathBuf {
    match r {
        Realised::Cached(p) | Realised::Generated(p) => p,
    }
}

#[test]
fn a_line_generates_once_as_a_wav_is_charged_once_and_is_cached() {
    let root = scratch("386_cache");
    let mock = Mock::default();
    let first = run(&root, false, &keyed(), &mock).unwrap();
    assert!(matches!(first, Realised::Generated(_)));
    let file = root.join(path(first.clone()));
    let bytes = std::fs::read(&file).unwrap();
    assert_eq!(
        &bytes[..4],
        b"RIFF",
        "the MP3 is converted to WAV on arrival"
    );
    assert_eq!(Sidecar::load(&file).unwrap().generated_on, "2026-10-05");

    let second = run(&root, false, &keyed(), &mock).unwrap();
    assert_eq!(second, Realised::Cached(path(first)));
    assert_eq!(mock.calls.get(), 1, "a cache hit calls nothing");
    // Flash, 22 characters: 1 cent, charged once.
    let ledger = Ledger::load(&root.join("provider_budget.json")).unwrap();
    assert_eq!(ledger.spent_cents, 1);
}

#[test]
fn the_request_carries_the_key_voice_and_vendor_model_id() {
    let mock = Mock::default();
    run(&scratch("386_request"), false, &keyed(), &mock).unwrap();
    let (key, voice, body) = mock.last.borrow().clone().unwrap();
    assert_eq!(
        (key.as_str(), voice.as_str()),
        (super::super::KEY, "voice-123")
    );
    assert_eq!(body.model_id, "eleven_flash_v2_5");
    assert_eq!(body.text, "Contact, second floor.");
}

#[test]
fn a_call_during_play_is_refused_even_for_a_cached_line() {
    let root = scratch("386_play");
    let mock = Mock::default();
    run(&root, false, &keyed(), &mock).unwrap();
    let refused = run(&root, true, &keyed(), &mock).unwrap_err();
    assert!(matches!(refused, SpeechError::Refused(Refusal::Playing)));
    assert_eq!(mock.calls.get(), 1);
}

#[test]
fn no_key_or_no_budget_spends_nothing() {
    let root = scratch("386_refused");
    let mock = Mock::default();
    let no_key = run(&root, false, &Environment::default(), &mock).unwrap_err();
    assert!(matches!(
        no_key,
        SpeechError::Refused(Refusal::MissingKey { .. })
    ));

    let ledger = Ledger {
        budget_cents: 0,
        spent_cents: 0,
    };
    ledger.save(&root.join("provider_budget.json")).unwrap();
    let over = run(&root, false, &keyed(), &mock).unwrap_err();
    assert!(matches!(
        over,
        SpeechError::Refused(Refusal::OverBudget { .. })
    ));
    assert_eq!(mock.calls.get(), 0);
}

#[test]
fn a_vendor_refusal_writes_nothing_and_charges_nothing() {
    let root = scratch("386_vendor");
    let mock = Mock {
        refuse: Some("ElevenLabs has no voice `voice-123`".into()),
        ..Mock::default()
    };
    let err = run(&root, false, &keyed(), &mock).unwrap_err();
    assert!(err.to_string().contains("voice-123"), "{err}");
    assert!(!root.join("project").exists(), "no file written");
    assert!(
        !root.join("provider_budget.json").exists(),
        "nothing charged"
    );
}
