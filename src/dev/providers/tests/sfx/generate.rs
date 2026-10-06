//! One effect through the boundary (#388), with a mock that answers with a committed
//! MP3 fixture and counts its calls. No test holds the real client.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

use super::super::super::generated::{Realised, Sidecar};
use super::super::super::sfx::*;
use super::super::super::{Context, Environment, Ledger, Refusal, Secret};
use super::super::{keyed, scratch};
use super::draft;

const MP3: &[u8] = include_bytes!("../../../../asset/audio/fixtures/loop.mp3");

#[derive(Default)]
struct Mock {
    calls: Cell<u32>,
    last: RefCell<Option<(String, Generate)>>,
    refuse: Option<String>,
}

impl Foley for Mock {
    fn make(&self, key: &Secret, body: &Generate) -> Result<Vec<u8>, String> {
        self.calls.set(self.calls.get() + 1);
        *self.last.borrow_mut() = Some((key.expose().into(), body.clone()));
        self.refuse.clone().map_or(Ok(MP3.to_vec()), Err)
    }
}

fn run(root: &Path, playing: bool, env: &Environment, mock: &Mock) -> Result<Realised, SfxError> {
    let ledger = root.join("provider_budget.json");
    let at = Context {
        root,
        playing,
        environment: env,
        ledger: &ledger,
        today: "2026-10-06",
    };
    generate(draft("heavy metal door slam", 12.0), &at, mock)
}

fn path(r: Realised) -> PathBuf {
    match r {
        Realised::Cached(p) | Realised::Generated(p) => p,
    }
}

#[test]
fn an_effect_generates_once_as_a_wav_is_charged_once_and_is_cached() {
    let root = scratch("388_cache");
    let mock = Mock::default();
    let first = run(&root, false, &keyed(), &mock).unwrap();
    assert!(matches!(first, Realised::Generated(_)));
    let file = root.join(path(first.clone()));
    assert!(path(first.clone()).to_string_lossy().contains("sfx-"));
    assert_eq!(&std::fs::read(&file).unwrap()[..4], b"RIFF");
    let sidecar = Sidecar::load(&file).unwrap();
    assert_eq!((sidecar.kind.as_str(), sidecar.estimate_cents), ("sfx", 5));

    let second = run(&root, false, &keyed(), &mock).unwrap();
    assert_eq!(second, Realised::Cached(path(first)));
    assert_eq!(mock.calls.get(), 1, "a cache hit calls nothing");
    // 12 s at 0.4¢: 4.8, rounded up to 5, charged once.
    let ledger = Ledger::load(&root.join("provider_budget.json")).unwrap();
    assert_eq!(ledger.spent_cents, 5);
}

#[test]
fn the_request_carries_the_key_length_and_model() {
    let mock = Mock::default();
    run(&scratch("388_request"), false, &keyed(), &mock).unwrap();
    let (key, body) = mock.last.borrow().clone().unwrap();
    assert_eq!(key, super::super::KEY);
    assert_eq!(body.text, "heavy metal door slam");
    assert!((body.duration_seconds - 12.0).abs() < f64::EPSILON);
    assert!((body.prompt_influence - 0.3).abs() < f64::EPSILON);
    assert_eq!((body.model_id.as_str(), body.looping), (MODEL, false));
}

#[test]
fn a_call_during_play_is_refused_even_when_cached() {
    let root = scratch("388_play");
    let mock = Mock::default();
    run(&root, false, &keyed(), &mock).unwrap();
    let refused = run(&root, true, &keyed(), &mock).unwrap_err();
    assert!(matches!(refused, SfxError::Refused(Refusal::Playing)));
    assert_eq!(mock.calls.get(), 1);
}

#[test]
fn no_key_or_no_budget_spends_nothing() {
    let root = scratch("388_refused");
    let mock = Mock::default();
    let no_key = run(&root, false, &Environment::default(), &mock).unwrap_err();
    assert!(matches!(
        no_key,
        SfxError::Refused(Refusal::MissingKey { .. })
    ));
    let broke = Ledger {
        budget_cents: 4,
        spent_cents: 0,
    };
    broke.save(&root.join("provider_budget.json")).unwrap();
    let over = run(&root, false, &keyed(), &mock).unwrap_err();
    assert!(matches!(
        over,
        SfxError::Refused(Refusal::OverBudget { .. })
    ));
    assert_eq!(mock.calls.get(), 0);
}

#[test]
fn a_vendor_refusal_writes_nothing_and_charges_nothing() {
    let root = scratch("388_vendor");
    let mock = Mock {
        refuse: Some("ElevenLabs: the account's plan does not cover this".into()),
        ..Mock::default()
    };
    let err = run(&root, false, &keyed(), &mock).unwrap_err();
    assert!(err.to_string().contains("plan"), "{err}");
    assert!(!root.join("assets").exists(), "no file written");
    assert!(
        !root.join("provider_budget.json").exists(),
        "nothing charged"
    );
}
