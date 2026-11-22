use std::process::Command; // lint: allow(subprocess) re-runs this test binary as a child

/// Set on the child: the test it runs alone.
const CHILD_VAR: &str = "RUSTY_TEST_ALONE";

/// What nextest sets in a test's process when it runs one test a process.
const NEXTEST_MODE: (&str, &str) = ("NEXTEST_EXECUTION_MODE", "process-per-test");

/// What the child prints, before the test's path, once the body has returned:
/// the parent passes only when it reads it.
const RAN: &str = "process::alone ran ";

#[macro_export]
macro_rules! alone {
    ($body:expr) => {{
        fn here() {}
        let name = ::std::any::type_name_of_val(&here);
        let test = name.strip_suffix("::here").expect("a function's path");
        $crate::process::alone(test, $body)
    }};
}

#[doc(hidden)]
pub fn alone(test: &str, body: impl FnOnce()) {
    let (_, path) = test.split_once("::").expect("a crate-rooted path");
    if let Err(said) = named(path, std::thread::current().name()) {
        panic!("{said}");
    }
    let own = std::env::var(NEXTEST_MODE.0).is_ok_and(|mode| mode == NEXTEST_MODE.1);
    let in_child = std::env::var(CHILD_VAR).is_ok_and(|child| child == path);
    if own || in_child {
        body();
        if in_child {
            println!("{RAN}{path}");
        }
        return;
    }
    if let Err(said) = child(path) {
        panic!("`{path}` failed alone:\n{said}");
    }
}

/// Whether `path` names the test running on `thread` (libtest names a test's
/// thread for its path; a thread it did not name proves nothing).
fn named(path: &str, thread: Option<&str>) -> Result<(), String> {
    match thread {
        Some(running) if running != "main" && running != path => Err(format!(
            "`process::alone` was given `{path}`, but the test running is `{running}`: \
             build the name with `rusty_test_support::alone!`"
        )),
        _ => Ok(()),
    }
}

/// Run the test at `path` (crate-relative) alone in a child of this binary: its
/// output when it failed or its body did not run.
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
    let ran = said
        .lines()
        .any(|line| line.ends_with(&format!("{RAN}{path}")));
    match out.status.success() && ran {
        true => Ok(()),
        false if out.status.success() => Err(format!(
            "its body did not run (is `test` its full path, written in `alone!`?):\n{said}"
        )),
        false => Err(said),
    }
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
