//! The opt-in itself, in real processes (#974): this test binary re-runs one of its
//! own tests as a child, with a key in its environment, once after calling
//! `read_keys` and once without. Only the first may resolve the key. A child, not
//! this process, because the opt-in is a process-wide flag no test may set here.

use std::process::Command; // lint: allow(subprocess) re-runs this test binary as a child

use super::super::credentials::{NEXTEST_VAR, NO_KEYS_VAR};
use super::*;

/// Set only in a child's environment: whether that child opts in.
const PROBE: &str = "RUSTY_KEY_PROBE";

/// What the child prints when its key resolved.
const RESOLVED: &str = "probe: the key resolved";

#[test]
fn opt_in_probe() {
    // A no-op unless a parent below started this process for it.
    let Some(mode) = std::env::var_os(PROBE) else {
        return;
    };
    if mode == "opted-in" {
        // lint: allow(read-keys) the child of the test below, proving the opt-in works
        read_keys();
    }
    if resolve(Provider::Gemini, &environment()).is_ok() {
        println!("{RESOLVED}");
    }
}

/// Run [`opt_in_probe`] in a child with a Gemini key set and `mode`; did it resolve?
fn child_resolves(mode: &str) -> bool {
    let (_, path) = module_path!()
        .split_once("::")
        .expect("a crate-rooted path");
    let mut child = Command::new(std::env::current_exe().expect("this test binary"));
    child.args([&format!("{path}::opt_in_probe"), "--exact", "--nocapture"]);
    // No real key a developer exported reaches the child: only the fake one.
    for provider in Provider::ALL {
        child.env_remove(provider.variable());
    }
    // The child runs as the libtest harness, outside nextest, so only the opt-in
    // decides; its working directory holds no `.env`.
    let out = child
        .env(PROBE, mode)
        .env(Provider::Gemini.variable(), KEY)
        .env_remove(NO_KEYS_VAR)
        .env_remove(NEXTEST_VAR)
        .current_dir(scratch(&format!("opt_in_{mode}")))
        .output()
        .expect("the test binary runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(
        stdout.contains("test result: ok. 1 passed;"),
        "the probe did not run: {stdout}"
    );
    stdout.contains(RESOLVED)
}

#[test]
fn a_process_that_opted_in_resolves_a_key() {
    assert!(child_resolves("opted-in"));
}

#[test]
fn a_process_that_never_opted_in_resolves_none() {
    assert!(!child_resolves("not"));
}
