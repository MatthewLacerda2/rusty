//! `key-guard`: every package whose tests link `rusty-dev` holds the provider-key
//! tripwire (#974).
//!
//! A process reads provider keys only once it opted in (`read-keys`), so no test
//! spends. Every package whose `[dependencies]` or `[dev-dependencies]` (per-target
//! ones too) name `rusty-dev` directly (the root, and each member crate that takes
//! it under `dev`) expands `rusty_dev::no_provider_keys!()` once in its code, `src/`
//! or `tests/`. `rusty-dev` itself is no such package: its own `providers` tests hold
//! the same facts.
//!
//! The guard is a **sample**, not a proof for every binary: cargo builds one test
//! binary per lib, per bin and per `tests/*.rs`, the guard sits in one of them, and a
//! package that links `rusty-dev` only through another sibling is not read. What
//! keeps every test process keyless is `read-keys` (only the three key readers opt in)
//! and the `NEXTEST` backstop (`dev::providers::credentials`); the sample proves that
//! holds inside a binary that links the provider layer. Nothing excuses it
//! (`Allow::Never`): a manifest line takes no inline allow.

use super::finding::Finding;
use super::scan::{names, Scan};
use crate::code::bare_lines;

pub const RULE: &str = "key-guard";

/// The macro a guarded package expands.
const GUARD: &str = "no_provider_keys!(";

/// Whether `source` expands the guard in code (not its definition, a comment or a
/// string).
fn expands(source: &str) -> bool {
    bare_lines(source).iter().any(|code| code.contains(GUARD))
}

/// A finding on `manifest` when it (or its tests) depends on `rusty-dev` and none of `sources`
/// expands the guard.
pub fn check<'a>(
    manifest: &str,
    text: &str,
    mut sources: impl Iterator<Item = &'a str>,
) -> Option<Finding> {
    (names(text, "rusty-dev") && !sources.any(expands)).then(|| Finding {
        path: manifest.to_string(),
        line: 1,
        rule: RULE,
        message: "this package depends on `rusty-dev`, so its test binary links the provider \
                  layer: expand `rusty_dev::no_provider_keys!();` once in its tests, under \
                  `#[cfg(all(test, feature = \"dev\"))]` (a member's `lib.rs`) or in `tests/`"
            .to_string(),
    })
}

/// The rule over the root package and every member crate: each manifest against the
/// code under its `src/` and `tests/`.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.packages()
        .iter()
        .filter_map(|p| check(&p.manifest, &p.text, p.files.iter().map(|f| scan.source(f))))
        .collect()
}

#[cfg(test)]
#[path = "key_guard_tests.rs"]
mod tests;
