//! The drain's fold and the logger's filter, unit by unit: which records
//! become one counted entry, and which a target and level let through.

use std::thread::ThreadId;

use super::*;

fn kept(origin: ThreadId, level: Level, text: &str, times: u32) -> Kept {
    let text = text.to_string();
    Kept {
        origin,
        level,
        text,
        times,
    }
}

#[test]
fn a_drain_folds_a_repeat_split_by_another_thread_into_one_counted_entry() {
    let me = std::thread::current().id();
    let other = std::thread::spawn(|| std::thread::current().id())
        .join()
        .unwrap();
    let mut pending = vec![
        kept(me, Level::Warn, "a", 2),
        kept(other, Level::Warn, "theirs", 1),
        kept(me, Level::Warn, "a", 3),
        kept(me, Level::Warn, "b", 1),
        kept(me, Level::Error, "b", 1),
    ];
    let runs = take_mine(&mut pending);
    let got: Vec<_> = runs
        .iter()
        .map(|k| (k.level, k.text.as_str(), k.times))
        .collect();
    let want = [
        (Level::Warn, "a", 5),
        (Level::Warn, "b", 1),
        (Level::Error, "b", 1),
    ];
    assert_eq!(
        got, want,
        "a repeat counts up; a new text or level is its own entry"
    );
    assert_eq!(
        pending.len(),
        1,
        "another thread's record stays for its own drain"
    );
}

#[test]
fn the_logger_lets_through_engine_warnings_and_what_stderr_wants() {
    let stderr = env_logger::Builder::new()
        .filter_level(LevelFilter::Error)
        .build();
    let logger = Forward { stderr };
    let at = |target: &str, level| {
        let meta = log::MetadataBuilder::new()
            .target(target)
            .level(level)
            .build();
        logger.enabled(&meta)
    };
    assert!(
        at("rusty::shell", Level::Warn),
        "the root's warnings reach the console"
    );
    assert!(
        at("rusty_logging::x", Level::Warn),
        "a workspace crate's too"
    );
    assert!(
        !at("rusty_core", Level::Info),
        "an engine info goes nowhere under `error`"
    );
    assert!(
        !at("wgpu_hal", Level::Warn),
        "a dependency's warning is stderr's call"
    );
    assert!(at("wgpu_hal", Level::Error), "and stderr takes its errors");
}
