//! A cache's freshness hands back what the engine wrote at once, a missing file
//! only once it appears, and nothing otherwise (#1241).

use super::{Freshness, MISS_RETRY_FRAMES};
use crate::core::fs::written::{note_written, Stale};

/// Poll `freshness` through one whole retry window; the stale paths of every poll.
fn window(freshness: &mut Freshness) -> Vec<String> {
    (0..MISS_RETRY_FRAMES)
        .flat_map(|_| freshness.poll().stale)
        .collect()
}

#[test]
fn a_write_is_handed_back_on_the_next_poll_only() {
    let mut freshness = Freshness::new();
    note_written("out/freshness_written.png");
    assert!(freshness
        .poll()
        .stale
        .contains(&"out/freshness_written.png".to_string()));
    assert!(!freshness
        .poll()
        .stale
        .contains(&"out/freshness_written.png".to_string()));
}

#[test]
fn a_miss_is_retried_once_its_file_appears_and_not_before() {
    let path = crate::test_temp::dir().join("freshness_miss.bin");
    std::fs::remove_file(&path).ok();
    let path = path.to_string_lossy().into_owned();
    let mut freshness = Freshness::new();
    freshness.miss(&path);
    assert!(!window(&mut freshness).contains(&path), "still missing");
    std::fs::write(&path, b"x").expect("write");
    assert!(window(&mut freshness).contains(&path), "appeared");
}

#[test]
fn the_retry_beat_comes_every_window() {
    let mut freshness = Freshness::new();
    let beats = (0..2 * MISS_RETRY_FRAMES).filter(|_| freshness.poll().retried);
    assert_eq!(beats.count(), 2);
}

#[test]
fn forgetting_a_stale_miss_reports_it() {
    let mut freshness = Freshness::new();
    freshness.miss("a.png");
    freshness.miss("b.png");
    assert!(!freshness.forget_misses(&Stale::new(vec!["c.png".into()])));
    assert!(freshness.forget_misses(&Stale::new(vec!["a.png".into()])));
    assert!(!freshness.is_miss("a.png") && freshness.is_miss("b.png"));
    freshness.retain_misses(|p| p != "b.png");
    assert_eq!(freshness.miss_count(), 0);
}

#[test]
fn a_cache_with_its_own_key_rules_says_what_appeared_and_what_is_stale() {
    let mut freshness = Freshness::new();
    freshness.miss("model.fbx#tex/Skin");
    let appeared = |p: &str| p.starts_with("model.fbx");
    let retried = (0..MISS_RETRY_FRAMES).flat_map(|_| freshness.poll_with(appeared).stale);
    assert!(retried.collect::<Vec<_>>().contains(&"model.fbx#tex/Skin".to_string()));
    assert!(freshness.forget_misses_where(|p| p.starts_with("model.fbx")));
    assert_eq!(freshness.miss_count(), 0);
}
