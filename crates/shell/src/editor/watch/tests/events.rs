//! What one event notes, fed by hand: a folder notes every file under it, a rescan
//! every file under the roots (#1327).

use super::{Watch, POLL};
use notify::event::{AccessKind, AccessMode, CreateKind, EventKind, Flag, ModifyKind};
use notify::Event;
use std::path::PathBuf;

/// A watch on a fresh folder holding `a.png` and `models/b.fbx`.
fn watched() -> (Watch, PathBuf) {
    let dir = rusty_test_support::temp::dir();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    std::fs::write(dir.join("a.png"), b"a").unwrap();
    std::fs::write(dir.join("models").join("b.fbx"), b"b").unwrap();
    (Watch::start(&[&dir], POLL).unwrap(), dir)
}

fn pending(watch: &Watch, dir: &std::path::Path, name: &str) -> bool {
    let key = dir.join(name);
    let key = key.to_string_lossy().into_owned();
    watch.pending().contains(&key)
}

/// inotify reports every open: a search over `assets/` must not wake the editor (#1721).
#[test]
fn a_read_is_not_worth_a_frame_but_a_change_or_lost_events_are() {
    use crate::editor::watch::matters;
    let on_file = |kind| Event::new(kind).add_path(PathBuf::from("a.png"));
    let read = EventKind::Access(AccessKind::Open(AccessMode::Any));
    assert!(!matters(&Ok(on_file(read))));
    assert!(matters(&Ok(on_file(EventKind::Modify(ModifyKind::Any)))));
    assert!(matters(&Ok(on_file(read).set_flag(Flag::Rescan))));
    assert!(matters(&Ok(Event::new(read))), "no path: the watch rescans");
    assert!(matters(&Err(notify::Error::generic("lost"))));
}

#[test]
fn a_folder_event_notes_every_file_under_it() {
    let (mut watch, dir) = watched();
    let folder = Event::new(EventKind::Create(CreateKind::Folder)).add_path(dir.join("models"));
    watch.take(&folder);
    assert!(
        pending(&watch, &dir, "models/b.fbx"),
        "{:?}",
        watch.pending()
    );
    assert!(!pending(&watch, &dir, "models"), "a folder is no asset");
    assert!(!pending(&watch, &dir, "a.png"), "only what is under it");
}

#[test]
fn a_rescan_or_a_pathless_event_notes_every_file_under_the_roots() {
    let (mut watch, dir) = watched();
    let rescan = Event::new(EventKind::Other).set_flag(Flag::Rescan);
    watch.take(&rescan);
    assert!(pending(&watch, &dir, "a.png"), "{:?}", watch.pending());
    assert!(pending(&watch, &dir, "models/b.fbx"));

    let (mut watch, dir) = watched();
    watch.take(&Event::new(EventKind::Modify(ModifyKind::Any)));
    assert!(pending(&watch, &dir, "a.png"), "{:?}", watch.pending());
    assert!(pending(&watch, &dir, "models/b.fbx"));
}
