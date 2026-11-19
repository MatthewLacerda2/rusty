//! A test that needs its process to itself: one reading a process-wide measure
//! (the peak memory) or installing process-wide state no other test may see (the
//! `log` logger). nextest already gives every test its own process; under plain
//! `cargo test` the whole binary shares one, so [`alone`] re-runs the test in a
//! child of this binary, by its exact name, and the body runs there.

use std::process::Command; // lint: allow(subprocess) re-runs this test binary as a child

/// Set on the child: the test it runs alone.
const CHILD_VAR: &str = "RUSTY_TEST_ALONE";

/// What nextest sets in a test's process when it runs one test a process.
const NEXTEST_MODE: (&str, &str) = ("NEXTEST_EXECUTION_MODE", "process-per-test");

/// Run `body` in a process that runs no other test. `test` is the test
/// function's full path, `concat!(module_path!(), "::", "<fn name>")`. Under
/// nextest, or in the child, `body` runs here; under plain `cargo test` the
/// binary re-runs `test` alone in a child, and a failure there fails this test
/// with the child's output.
pub fn alone(test: &str, body: impl FnOnce()) {
    let (_, path) = test.split_once("::").expect("a crate-rooted path");
    let own = std::env::var(NEXTEST_MODE.0).is_ok_and(|mode| mode == NEXTEST_MODE.1);
    if own || std::env::var(CHILD_VAR).is_ok_and(|child| child == path) {
        body();
        return;
    }
    if let Err(said) = child(path) {
        panic!("`{path}` failed alone:\n{said}");
    }
}

/// Run the test at `path` (crate-relative) alone in a child of this binary: its
/// output when it failed or did not run.
fn child(path: &str) -> Result<(), String> {
    let out = Command::new(std::env::current_exe().expect("this test binary"))
        .args([path, "--exact", "--nocapture", "--include-ignored"])
        .env(CHILD_VAR, path)
        .output()
        .expect("the test binary runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let ran = said.contains("test result: ok. 1 passed;");
    match out.status.success() && ran {
        true => Ok(()),
        false if out.status.success() => Err(format!(
            "it did not run (is `test` its full path?):\n{said}"
        )),
        false => Err(said),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Bumped by the body of [`a_body_runs_once_alone`] wherever it runs.
    static RAN: AtomicUsize = AtomicUsize::new(0);

    const PROBE: &str = "process::tests::a_failing_probe";

    #[test]
    fn a_body_runs_once_alone() {
        alone(concat!(module_path!(), "::a_body_runs_once_alone"), || {
            RAN.fetch_add(1, Ordering::SeqCst);
            assert_eq!(RAN.load(Ordering::SeqCst), 1);
        });
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

    #[test]
    fn a_failing_child_is_reported() {
        let said = child(PROBE).unwrap_err();
        assert!(said.contains("the probe failed"), "{said}");
        let missing = child("process::tests::no_such_test").unwrap_err();
        assert!(missing.contains("did not run"), "{missing}");
    }
}
