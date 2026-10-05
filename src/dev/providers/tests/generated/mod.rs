//! A generated asset is addressed by its brief (#384), pinned with a fake provider
//! whose "generation" is a constant. No network anywhere.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::super::generated::*;

mod canonical;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct Line {
    pub text: String,
    pub voice: String,
    pub speed: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    pub tags: BTreeMap<String, String>,
}

impl Brief for Line {
    fn kind(&self) -> &'static str {
        "speech"
    }
    fn extension(&self) -> &'static str {
        "mp3"
    }
}

pub(super) fn line(text: &str) -> Line {
    let tags = BTreeMap::from([("mood".into(), "grim".into())]);
    Line {
        text: text.into(),
        voice: "v1".into(),
        speed: 1.1,
        seed: None,
        tags,
    }
}

pub(super) fn root(tag: &str) -> PathBuf {
    super::scratch(&format!("384_{tag}"))
}

/// Realise `brief`, counting the provider calls the fake receives.
fn realise_counting(root: &std::path::Path, brief: &Line, calls: &Cell<u32>) -> Realised {
    realise(root, brief, "2026-10-05", |_| {
        calls.set(calls.get() + 1);
        Ok::<_, std::io::Error>((b"mp3".to_vec(), 7))
    })
    .unwrap()
}

#[test]
fn a_brief_generates_once_and_is_cached_the_second_time() {
    let root = root("once");
    let calls = Cell::new(0);
    let first = realise_counting(&root, &line("Get down!"), &calls);
    let second = realise_counting(&root, &line("Get down!"), &calls);
    let Realised::Generated(path) = first else {
        panic!("first call generates: {first:?}")
    };
    assert_eq!(second, Realised::Cached(path.clone()));
    assert_eq!(calls.get(), 1, "the cache hit must not call the provider");
    assert_eq!(std::fs::read(root.join(&path)).unwrap(), b"mp3");
}

#[test]
fn the_address_is_kind_digest_extension_under_the_generated_dir() {
    let brief = line("Reload!");
    let path = address(&brief).unwrap();
    let name = path.file_name().unwrap().to_str().unwrap();
    assert_eq!(path.parent().unwrap(), std::path::Path::new(GENERATED_DIR));
    assert_eq!(name, format!("speech-{}.mp3", digest(&brief).unwrap()));
    assert_eq!(digest(&brief).unwrap().len(), 64, "sha256, lowercase hex");
}

#[test]
fn a_changed_brief_goes_stale_and_a_reverted_one_hits_the_original_entry() {
    let root = root("stale");
    let calls = Cell::new(0);
    let Realised::Generated(first) = realise_counting(&root, &line("Flank left"), &calls) else {
        panic!("expected a generation")
    };
    let edited = line("Flank right");
    assert_eq!(
        state(&root, &edited, Some(&first)).unwrap(),
        GenerationState::Stale
    );
    assert_eq!(calls.get(), 1, "going stale never regenerates");
    let reverted = line("Flank left");
    assert_eq!(
        state(&root, &reverted, Some(&first)).unwrap(),
        GenerationState::Generated
    );
    assert_eq!(
        realise_counting(&root, &reverted, &calls),
        Realised::Cached(first)
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn a_sketch_reports_as_a_sketch_and_spends_nothing() {
    let root = root("sketch");
    let brief = line("Not paid for yet");
    assert_eq!(state(&root, &brief, None).unwrap(), GenerationState::Sketch);
    assert_eq!(cached(&root, &brief).unwrap(), None);
    assert!(
        !root.join(GENERATED_DIR).exists(),
        "a sketch writes nothing"
    );
}
