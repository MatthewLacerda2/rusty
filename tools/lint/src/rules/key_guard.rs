//! `key-guard`: every test binary that links `rusty-dev` holds the provider-key
//! tripwire (#974).
//!
//! A process reads provider keys only once it opted in (`read-keys`), so no test
//! spends; each test binary proves it of itself. A package's tests build one binary
//! per package, so every package whose dependencies or dev-dependencies name
//! `rusty-dev` (the root, and each member crate that takes it under `dev`) expands
//! `rusty_dev::no_provider_keys!()` once in its code, `src/` or `tests/`. `rusty-dev`
//! itself is no such package: its own `providers` tests hold the same facts. There is
//! no exception: the guard is two tests.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::bare_lines;
use std::path::Path;

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
    let mut named = crate::layers::manifest_dependencies(text);
    named.extend(crate::layers::manifest_dev_dependencies(text));
    let links = named.iter().any(|k| k == "rusty_dev");
    (links && !sources.any(expands)).then(|| Finding {
        path: manifest.to_string(),
        line: 1,
        rule: RULE,
        message: "this package depends on `rusty-dev`, so its test binary links the provider \
                  layer: expand `rusty_dev::no_provider_keys!();` once in its tests, under \
                  `#[cfg(all(test, feature = \"dev\"))]` (a member's `lib.rs`) or in `tests/`"
            .to_string(),
    })
}

/// The rule over the root package and every member crate.
pub fn run(scan: &Scan) -> Vec<Finding> {
    let mut packages = vec![(String::from("Cargo.toml"), vec!["src/", "tests/"])];
    for manifest in crate::modules::member_manifests(Path::new("")) {
        let at = manifest.to_string_lossy().replace('\\', "/");
        packages.push((at, Vec::new()));
    }
    let mut found = Vec::new();
    for (manifest, dirs) in packages {
        let dirs = if dirs.is_empty() {
            let root = manifest.trim_end_matches("Cargo.toml");
            vec![format!("{root}src/"), format!("{root}tests/")]
        } else {
            dirs.into_iter().map(String::from).collect()
        };
        let text = std::fs::read_to_string(&manifest).unwrap_or_default();
        let inside = |f: &&String| dirs.iter().any(|d| f.starts_with(d.as_str()));
        let sources = scan.files.iter().filter(inside).map(|f| scan.source(f));
        found.extend(check(&manifest, &text, sources));
    }
    found
}

#[cfg(test)]
#[path = "key_guard_tests.rs"]
mod tests;
