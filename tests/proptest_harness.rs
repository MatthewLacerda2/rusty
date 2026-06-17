//! Property-based harness invariants (#123). Gated on `dev` like
//! `harness_determinism.rs`: a no-op under a plain `cargo test`, real under
//! `--features dev`. Strengthens the single-case replay/advancement checks into
//! invariants over generated step counts.

#![cfg(feature = "dev")]

use proptest::prelude::*;
use rusty::dev::harness::Harness;

fn dir(tag: u64) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("rusty_proptest_harness_{tag}"))
}

proptest! {
    // Bound step counts so the suite stays fast; the property holds for any count.
    #![proptest_config(ProptestConfig { cases: 24, ..ProptestConfig::default() })]

    /// Fixed-step advancement is exact and monotonic: after `n` steps the frame
    /// counter is exactly `n`, and splitting the steps (`a` then `b`) lands on the
    /// same frame as one `a + b` run.
    #[test]
    fn stepping_advances_monotonically(a in 0u32..200, b in 0u32..200) {
        let split = Harness::new(dir(1), "");
        split.step(a);
        prop_assert_eq!(split.frame(), u64::from(a));
        split.step(b);
        prop_assert_eq!(split.frame(), u64::from(a + b));

        let combined = Harness::new(dir(2), "");
        combined.step(a + b);
        prop_assert_eq!(combined.frame(), u64::from(a + b));
    }

    /// The sim is a pure function of (inputs, fixed dt): two harness runs of the
    /// same step count produce byte-identical snapshots, for any step count.
    #[test]
    fn replay_is_byte_identical(steps in 0u32..250) {
        let run = || {
            let h = Harness::new(dir(3), "");
            h.step(steps);
            h.snapshot().to_string()
        };
        prop_assert_eq!(run(), run());
    }
}
