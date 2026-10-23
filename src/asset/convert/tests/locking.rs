//! The cache's lock (#1166): on a lock file of its own, not the source, taken
//! only to convert, and kept out of `assets/` (#1190).

use super::super::{cache_path, lock, record, refresh};
use super::COPY;
use crate::core::{paths, project};
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

#[test]
fn a_save_while_waiting_for_the_lock_is_what_gets_stamped() {
    let (source, out) = cached();
    let held = lock::hold(&out).expect("this filesystem locks");
    std::fs::write(&source, "two").unwrap();
    let finished = refresh_elsewhere(&source, &out);
    // The waiter read "two" before blocking; the exporter saves again meanwhile.
    let early = finished.recv_timeout(Duration::from_millis(200));
    assert!(early.is_err(), "converted while the lock was held");
    std::fs::write(&source, "three").unwrap();
    drop(held);
    assert_eq!(finished.recv_timeout(Duration::from_secs(10)), Ok(true));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "three");
    let stamp = record::stamp(&COPY, crate::core::hash::fnv1a64(b"three"));
    assert_eq!(
        record::current(&out, &stamp).map(|stored| stored.skipped),
        Some(Vec::new()),
        "stamped as what it holds"
    );
}

/// Every file under `dir`, at any depth.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let paths = entries.flatten().map(|entry| entry.path());
    paths
        .flat_map(|p| if p.is_dir() { files_under(&p) } else { vec![p] })
        .collect()
}

#[test]
fn a_projects_lock_lives_in_its_cache_folder_never_in_assets() {
    let root = crate::test_temp::dir();
    std::fs::write(root.join(project::PROJECT_FILE), "").unwrap();
    std::fs::create_dir_all(root.join("assets/models")).unwrap();
    let source = root.join("assets/models/a.src");
    std::fs::write(&source, "one").unwrap();
    let out = cache_path(&root, Path::new("assets/models/a.src"));
    refresh(&COPY, &source, &out).unwrap();
    std::fs::write(&source, "two").unwrap();
    refresh(&COPY, &source, &out).unwrap();

    let assets = files_under(&root.join(project::ASSETS_DIR));
    let is_lock = |p: &PathBuf| p.extension().is_some_and(|e| e == "lock");
    assert!(!assets.iter().any(is_lock), "{assets:?}");
    // Canonical, as `lock::path` is: macOS's temp dir `/var` is a link to `/private/var`.
    let locks = paths::canonicalize(&root.join(project::CACHE_DIR).join(lock::LOCKS_DIR));
    let locks = files_under(&locks.unwrap());
    assert_eq!(locks, vec![lock::path(&out).unwrap()], "one lock, reused");
}

#[test]
fn two_spellings_of_one_cache_share_its_lock() {
    let dir = crate::test_temp::dir();
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let plain = lock::path(&dir.join("a.src.glb")).unwrap();
    assert_eq!(lock::path(&dir.join("sub/../a.src.glb")).unwrap(), plain);
    assert_ne!(lock::path(&dir.join("b.src.glb")).unwrap(), plain);
}
