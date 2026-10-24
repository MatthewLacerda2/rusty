//! The watch notes a change made outside the engine, once, after Play (#1327).
//! Driven with `notify` on a temp folder: the polling backend always, the native one
//! wherever the filesystem gives events.

use super::{changes, Backend, Root, Watch};
use crate::core::fs::written::written_since;
use notify::event::{AccessKind, AccessMode, CreateKind, EventKind, ModifyKind};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const POLL: Backend = Backend::Poll(Duration::from_millis(20));

/// Collect until `path` is pending, or give up after a few seconds.
fn waits_for(watch: &mut Watch, path: &Path) -> bool {
    let key = path.to_string_lossy().into_owned();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        watch.collect();
        if watch.pending().contains(&key) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// A watch on a fresh folder holding `texture.png`, and that file's path.
fn watched(backend: Backend) -> (Watch, PathBuf) {
    let dir = crate::test_temp::dir();
    let file = dir.join("texture.png");
    std::fs::write(&file, b"red").unwrap();
    (Watch::start(&[&dir], backend).unwrap(), file)
}

/// Change `file` outside the engine, then check the watch notes it only after Play.
fn notes_an_outside_change_after_play(mut watch: Watch, file: &Path) -> bool {
    // The poller reads modification times to the second: move into the next one.
    std::thread::sleep(Duration::from_millis(1100));
    std::fs::write(file, b"a longer blue").unwrap();
    if !waits_for(&mut watch, file) {
        return false;
    }
    let key = file.to_string_lossy().into_owned();
    let (start, _) = written_since(u64::MAX);
    assert_eq!(watch.apply(true), 0, "nothing is noted during Play");
    assert!(watch.pending().contains(&key), "held for Stop");
    assert!(!written_since(start).1.contains(&key));
    assert!(watch.apply(false) >= 1);
    assert!(written_since(start).1.contains(&key), "noted after Stop");
    assert!(watch.pending().is_empty(), "noted once");
    true
}

#[test]
fn the_polling_watch_notes_an_outside_change() {
    let (watch, file) = watched(POLL);
    assert!(notes_an_outside_change_after_play(watch, &file));
}

#[test]
fn the_native_watch_notes_an_outside_change() {
    let (watch, file) = watched(Backend::Native);
    let noted = notes_an_outside_change_after_play(watch, &file);
    // inotify and FSEvents both report a temp folder's writes.
    assert!(noted, "no event from the native backend");
}

#[test]
fn a_file_appearing_is_noted_and_a_write_in_flight_is_not() {
    let (mut watch, file) = watched(POLL);
    let temp = crate::core::fs::unique_sibling(&file, crate::core::fs::TEMP_SUFFIX);
    std::fs::write(&temp, b"half").unwrap();
    let new = file.with_file_name("new.png");
    std::fs::write(&new, b"x").unwrap();
    assert!(waits_for(&mut watch, &new), "a new file is a change");
    let temp = temp.to_string_lossy().into_owned();
    assert!(!watch.pending().contains(&temp), "{temp} is never an asset");
}

#[test]
fn a_change_under_an_ignored_folder_is_not_noted() {
    let (mut watch, file) = watched(POLL);
    let cache = file.with_file_name("imported");
    std::fs::create_dir(&cache).unwrap();
    watch.ignored.push(cache.clone());
    std::fs::write(cache.join("model.glb"), b"x").unwrap();
    let after = file.with_file_name("after.png");
    std::fs::write(&after, b"x").unwrap();
    assert!(waits_for(&mut watch, &after));
    let cached = |p: &String| Path::new(p).starts_with(&cache);
    assert!(!watch.pending().iter().any(cached), "{:?}", watch.pending());
}

#[test]
fn a_path_is_spelled_under_its_root_as_given() {
    let root = Root::new(Path::new("assets"));
    let cwd = std::env::current_dir().unwrap();
    let spelled = root.spell(&cwd.join("assets/textures/a.png"));
    assert_eq!(spelled, Some(PathBuf::from("assets/textures/a.png")));
    assert_eq!(root.spell(&cwd.join("elsewhere/a.png")), None);
}

#[test]
fn only_events_that_can_change_a_file_count() {
    assert!(changes(&EventKind::Modify(ModifyKind::Any)));
    assert!(changes(&EventKind::Create(CreateKind::File)));
    assert!(changes(&EventKind::Access(AccessKind::Close(
        AccessMode::Write
    ))));
    assert!(!changes(&EventKind::Access(AccessKind::Open(
        AccessMode::Any
    ))));
    assert!(!changes(&EventKind::Access(AccessKind::Read)));
}
