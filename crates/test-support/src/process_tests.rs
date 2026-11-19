use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Bumped by the body of [`a_body_runs_once_alone`] wherever it runs.
static BODIES: AtomicUsize = AtomicUsize::new(0);

const PROBE: &str = "process::tests::a_failing_probe";

/// Whether `alone` runs its body in this process rather than in a child.
fn in_place(path: &str) -> bool {
    std::env::var(NEXTEST_MODE.0).is_ok_and(|mode| mode == NEXTEST_MODE.1)
        || std::env::var(CHILD_VAR).is_ok_and(|child| child == path)
}

#[test]
fn a_body_runs_once_alone() {
    crate::alone!(|| {
        BODIES.fetch_add(1, Ordering::SeqCst);
    });
    // Here (nextest, or the child) the body ran once; in the parent it ran in the
    // child, whose run of this same check the parent required to pass.
    let here = in_place("process::tests::a_body_runs_once_alone");
    assert_eq!(BODIES.load(Ordering::SeqCst), usize::from(here));
}

#[test]
fn a_stale_name_is_refused_before_anything_runs() {
    let stale = concat!(module_path!(), "::a_test_renamed_since");
    let refused = std::panic::catch_unwind(|| alone(stale, || panic!("the body ran")));
    let said = refused.expect_err("a stale name panics");
    let said = said.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(said.contains("rusty_test_support::alone!"), "{said}");
    let path = "process::tests::a";
    assert!(named(path, Some("process::tests::b")).is_err());
    assert!(named(path, Some(path)).is_ok());
    assert!(named(path, Some("main")).is_ok() && named(path, None).is_ok());
}

/// Fails only in the child [`a_failing_child_is_reported`] starts.
#[test]
// lint: allow(skip) the child half: a_failing_child_is_reported runs it
#[ignore = "run by a_failing_child_is_reported in a child process"]
fn a_failing_probe() {
    if std::env::var(CHILD_VAR).is_ok_and(|child| child == PROBE) {
        panic!("the probe failed");
    }
}

/// Passes without running a body through `alone`.
#[test]
// lint: allow(skip) the child half: a_failing_child_is_reported runs it
#[ignore = "run by a_failing_child_is_reported in a child process"]
fn a_probe_without_a_body() {}

#[test]
fn a_failing_child_is_reported() {
    let said = child(PROBE).unwrap_err();
    assert!(said.contains("the probe failed"), "{said}");
    let missing = child("process::tests::no_such_test").unwrap_err();
    assert!(missing.contains("did not run"), "{missing}");
    let skipped = child("process::tests::a_probe_without_a_body").unwrap_err();
    assert!(skipped.contains("did not run"), "{skipped}");
}
