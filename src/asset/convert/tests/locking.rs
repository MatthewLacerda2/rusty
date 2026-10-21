//! The cache's lock (#1166): on a `<cache>.lock` file, not the source, and taken
//! only to convert.

use super::super::{lock, refresh};
use super::COPY;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// `source` refreshed into `out` on another thread, as another process would;
/// the receiver gets its result.
fn refresh_elsewhere(source: &Path, out: &Path) -> Receiver<bool> {
    let (source, out) = (source.to_path_buf(), out.to_path_buf());
    let (done, finished) = channel();
    std::thread::spawn(move || done.send(refresh(&COPY, &source, &out).is_ok()));
    finished
}

/// A source converted once from "one", and its cache.
fn cached() -> (PathBuf, PathBuf) {
    let dir = crate::test_temp::dir();
    let (source, out) = (dir.join("a.src"), dir.join("a.src.glb"));
    std::fs::write(&source, "one").unwrap();
    refresh(&COPY, &source, &out).unwrap();
    (source, out)
}

#[test]
fn a_current_cache_never_waits_on_a_held_lock() {
    let (source, out) = cached();
    let _held = lock::hold(&out).expect("this filesystem locks");
    let finished = refresh_elsewhere(&source, &out);
    assert_eq!(finished.recv_timeout(Duration::from_secs(10)), Ok(true));
}

#[test]
fn a_source_replaced_by_rename_still_waits_for_the_lock() {
    let (source, out) = cached();
    let held = lock::hold(&out).expect("this filesystem locks");
    // An exporter's save: a new file renamed over the source.
    let saved = source.with_extension("tmp");
    std::fs::write(&saved, "two").unwrap();
    // lint: allow(atomic-write) the test plays an exporter saving by rename
    std::fs::rename(&saved, &source).unwrap();

    let finished = refresh_elsewhere(&source, &out);
    let early = finished.recv_timeout(Duration::from_millis(200));
    assert!(early.is_err(), "converted while the lock was held");
    drop(held);
    assert_eq!(finished.recv_timeout(Duration::from_secs(10)), Ok(true));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "two");
}
