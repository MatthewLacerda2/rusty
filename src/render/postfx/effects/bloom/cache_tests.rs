//! Bloom's dirt cache (#1442): each path is read once until it changes on disk, a
//! missing one is retried once it appears, an undecodable one only once written.

use std::cell::Cell;

use super::DirtCache;
use crate::core::fs::freshness::MISS_RETRY_FRAMES;
use crate::core::fs::written::note_written;

/// `frames` frames of `cache` asking for `setting`, counting the reads; a read
/// succeeds when the file decodes as text `"ok"`.
fn frames(cache: &mut DirtCache<u8>, setting: &str, frames: u64) -> u32 {
    let reads = Cell::new(0);
    for _ in 0..frames {
        cache.refresh();
        cache.request(setting, |file| {
            reads.set(reads.get() + 1);
            let text = std::fs::read_to_string(file.file()?).ok()?;
            (text == "ok").then_some(1)
        });
    }
    reads.get()
}

#[test]
fn an_undecodable_dirt_is_read_once_until_it_is_written() {
    let path = crate::test_temp::dir().join("dirt.png");
    std::fs::write(&path, "garbage").unwrap();
    let setting = path.to_string_lossy().into_owned();
    let mut cache = DirtCache::default();
    assert_eq!(frames(&mut cache, &setting, 3 * MISS_RETRY_FRAMES), 1);
    assert_eq!(cache.get(&setting), None);
    std::fs::write(&path, "ok").unwrap();
    note_written(&setting);
    assert_eq!(frames(&mut cache, &setting, 2), 1, "a write reads it again");
    assert_eq!(cache.get(&setting), Some(1));
}

#[test]
fn a_missing_dirt_is_retried_only_once_it_appears() {
    let path = crate::test_temp::dir().join("dirt.png");
    let setting = path.to_string_lossy().into_owned();
    let mut cache = DirtCache::default();
    assert_eq!(frames(&mut cache, &setting, 3 * MISS_RETRY_FRAMES), 1);
    std::fs::write(&path, "ok").unwrap();
    assert_eq!(frames(&mut cache, &setting, MISS_RETRY_FRAMES), 1);
    assert_eq!(cache.get(&setting), Some(1));
}

#[test]
fn a_render_texture_dirt_is_never_read_from_disk() {
    let mut cache = DirtCache::<u8>::default();
    assert_eq!(frames(&mut cache, "rt:mirror", 2), 0);
}
