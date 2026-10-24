//! The engine check (#853, #1319): which way the engine moved, and what each way
//! says.

use super::*;

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

/// A project that recorded `recorded` at `old`, opened by `running` at `new`.
fn check(recorded: Option<&str>, old: Option<u64>, running: &str, new: Option<u64>) -> EngineCheck {
    EngineCheck {
        recorded: recorded.map(str::to_string),
        recorded_order: old,
        running: running.to_string(),
        running_order: new,
    }
}

#[test]
fn the_verdict_follows_the_count() {
    let verdict = |old, new, running: &str| check(Some(A), old, running, new).verdict();
    assert_eq!(verdict(Some(5), Some(5), A), Verdict::Same);
    assert_eq!(
        verdict(Some(5), Some(9), &format!("{A}-dirty")),
        Verdict::Same
    );
    assert_eq!(verdict(Some(5), Some(6), B), Verdict::Upgrade);
    assert_eq!(
        verdict(None, Some(6), B),
        Verdict::Upgrade,
        "a file from before counts"
    );
    assert_eq!(verdict(Some(6), Some(5), B), Verdict::Older);
    assert_eq!(
        verdict(Some(5), Some(5), B),
        Verdict::Diverged,
        "a branch build"
    );
    assert_eq!(
        verdict(Some(5), None, B),
        Verdict::Unknown,
        "a shallow clone can't count"
    );
    assert_eq!(verdict(Some(5), Some(1), UNKNOWN_COMMIT), Verdict::Unknown);
    let unknown_record = check(Some(UNKNOWN_COMMIT), Some(9), A, Some(1));
    assert_eq!(unknown_record.verdict(), Verdict::Unknown);
    assert_eq!(
        check(None, None, A, Some(1)).verdict(),
        Verdict::Same,
        "nothing recorded"
    );
}

#[test]
fn only_an_older_engine_or_another_line_warns() {
    assert_eq!(
        check(Some(A), Some(5), B, Some(6)).warning(),
        None,
        "an upgrade"
    );
    assert_eq!(check(Some(A), Some(5), B, None).warning(), None, "unknown");
    assert_eq!(check(Some(A), None, A, None).warning(), None, "the same");
    assert!(
        check(Some(A), Some(5), B, Some(5)).warning().is_some(),
        "diverged"
    );
    let line = check(Some(A), Some(6), &format!("{B}-dirty"), Some(5));
    let line = line.warning().unwrap();
    assert!(line.contains("aaaaaaaaaaaa,"), "{line}");
    assert!(line.contains("bbbbbbbbbbbb-dirty:"), "{line}");
    assert!(!line.contains(A), "commits are shortened: {line}");
}

#[test]
fn an_upgrade_names_the_log_of_what_changed() {
    let up = check(
        Some(&format!("{A}-dirty")),
        Some(5),
        &format!("{B}-dirty"),
        Some(6),
    );
    let line = up.upgrade().unwrap();
    let (old, new) = (&A[..12], &B[..12]);
    assert!(
        line.starts_with(&format!("upgraded from engine {old} to {new};")),
        "{line}"
    );
    assert!(
        line.ends_with(&format!(" log --oneline {old}..{new}")),
        "{line}"
    );
    let checkout = crate::core::project::engine_dir().parent().unwrap();
    assert!(
        line.contains(&format!("git -C {} ", checkout.display())),
        "{line}"
    );
    assert_eq!(check(Some(A), Some(6), B, Some(5)).upgrade(), None, "older");
    assert_eq!(
        check(Some(A), Some(5), A, Some(6)).upgrade(),
        None,
        "the same"
    );
}

#[test]
fn an_empty_record_is_no_record_and_the_running_engine_is_this_one() {
    commit::pin(B, Some(3));
    let check = EngineCheck::against(Some(""), Some(2));
    assert_eq!((check.recorded, check.recorded_order), (None, Some(2)));
    let check = EngineCheck::against(Some(A), None);
    assert_eq!(check.recorded.as_deref(), Some(A));
    assert_eq!((check.running.as_str(), check.running_order), (B, Some(3)));
}
