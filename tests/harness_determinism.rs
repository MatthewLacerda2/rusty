//! Headless harness determinism + control-surface smoke (issue #3).
//! Gated on `dev`: a no-op under a plain `cargo test`, real under `--features dev`.
#![cfg(feature = "dev")]

use rusty::dev::harness::Harness;

#[test]
fn step_advances_fixed_frames() {
    let mut h = Harness::new(std::env::temp_dir().join("rusty_test_a"), "");
    h.step(120);
    assert_eq!(h.frame(), 120);
}

#[test]
fn fixed_timestep_replay_is_deterministic() {
    let run = || {
        let mut h = Harness::new(std::env::temp_dir().join("rusty_test_b"), "");
        h.step(200);
        h.snapshot().to_string()
    };
    assert_eq!(run(), run(), "same steps must yield identical snapshots");
}

#[test]
fn expect_failures_are_recorded() {
    let mut h = Harness::new(std::env::temp_dir().join("rusty_test_c"), "");
    h.expect(true, "ok".into());
    assert!(h.all_passed());
    h.expect(false, "boom".into());
    assert!(!h.all_passed());
}
