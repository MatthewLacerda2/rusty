//! A converter that runs a program (#1072): the program's version is in the
//! stamp, a refresh asks it once, Play never does, and with no program a cache of
//! the unchanged source stands.

use super::super::{gate, record, refresh, Converter};
use crate::asset::ImportError;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

thread_local! {
    /// The fake program's version; `None` while it is not installed.
    static VERSION: RefCell<Option<String>> = const { RefCell::new(None) };
    /// How often it was asked.
    static ASKED: Cell<usize> = const { Cell::new(0) };
}

fn tool() -> Result<String, ImportError> {
    ASKED.set(ASKED.get() + 1);
    let version = VERSION.with_borrow(Clone::clone);
    version.ok_or_else(|| ImportError::Parse("no program found: install it".into()))
}

/// A converter that copies its source, through the fake program.
const TOOLED: Converter = Converter {
    tool: Some(tool),
    ..super::COPY
};

fn install(version: Option<&str>) {
    VERSION.set(version.map(Into::into));
}

/// A source reading `one`, converted with program version `5.0.1`; the cache
/// then made to read `kept`, so a test sees whether it is converted again.
fn converted() -> (PathBuf, PathBuf) {
    let dir = crate::test_temp::dir();
    let (source, out) = (dir.join("a.src"), dir.join("a.src.glb"));
    std::fs::write(&source, "one").unwrap();
    install(Some("5.0.1"));
    refresh(&TOOLED, &source, &out).unwrap();
    std::fs::write(&out, "kept").unwrap();
    (source, out)
}

fn read(out: &Path) -> String {
    std::fs::read_to_string(out).unwrap()
}

#[test]
fn a_different_program_version_converts_again() {
    let (source, out) = converted();
    refresh(&TOOLED, &source, &out).unwrap();
    assert_eq!(read(&out), "kept", "the same version: current");
    install(Some("5.1.0"));
    refresh(&TOOLED, &source, &out).unwrap();
    assert_eq!(read(&out), "one", "converted again");
    let stamp = read(&record::stamp_path(&out));
    assert!(stamp.lines().next().unwrap().ends_with(" 5.1.0"), "{stamp}");
}

#[test]
fn with_no_program_an_unchanged_source_keeps_its_cache() {
    let (source, out) = converted();
    install(None);
    refresh(&TOOLED, &source, &out).expect("the cache stands");
    assert_eq!(read(&out), "kept");
}

#[test]
fn with_no_program_a_changed_source_says_what_is_missing() {
    let (source, out) = converted();
    install(None);
    std::fs::write(&source, "two").unwrap();
    let why = refresh(&TOOLED, &source, &out).expect_err("nothing converts it");
    assert!(why.to_string().contains("install it"), "{why}");
    assert_eq!(read(&out), "kept", "the old cache is left as it was");
}

#[test]
fn a_refresh_asks_the_program_once() {
    ASKED.set(0);
    let (source, out) = converted();
    assert_eq!(ASKED.get(), 1, "a conversion");
    refresh(&TOOLED, &source, &out).unwrap();
    assert_eq!(ASKED.get(), 2, "a check of a current cache");
}

#[test]
fn play_serves_a_cache_without_asking_the_program() {
    let (source, out) = converted();
    ASKED.set(0);
    install(Some("6.0.0"));
    gate::serve(&TOOLED, &source, &out).expect("served");
    assert_eq!(ASKED.get(), 0);
    assert_eq!(read(&out), "kept");
}

#[test]
fn a_missing_source_is_served_from_its_cache_without_asking_the_program() {
    let (source, out) = converted();
    std::fs::remove_file(&source).unwrap();
    ASKED.set(0);
    refresh(&TOOLED, &source, &out).expect("the cache stands");
    assert_eq!(ASKED.get(), 0, "a player without sources never starts it");
    assert_eq!(read(&out), "kept");
}
