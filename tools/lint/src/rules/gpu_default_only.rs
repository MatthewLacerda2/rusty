//! `gpu-default-only`: no code a GPU test can reach exists only without `dev`.
//!
//! The default-feature test suite leaves the `gpu` test group to the `dev` suite
//! (`make test`'s `DEFAULT_SUITE`, CI's "Test (engine, default)" step; docs/testing.md,
//! *Which suite runs the GPU tests*). That is sound only while `dev` adds to the
//! default build and never takes code away from what is drawn: code that compiles
//! only without `dev` is drawn by no GPU test any more. So in a crate in scope
//! ([`scope`]: `rusty-render`, every crate its `dev` turns on, and every crate with an
//! in-crate `gpu_` test), a production line holding a `cfg!(…)` that names
//! `feature = "dev"` (a value that differs by feature set), or any `cfg(…)` /
//! `cfg_attr(…)` attribute whose `not(…)` names it, wrapped over lines or not, is a
//! finding. Test code counts for the attribute too: a GPU test compiled only without
//! `dev` runs in neither suite (a test's `cfg!` stays exempt: it reads, never hides).
//! `any(dev, editor)` is not: `editor` is a default feature, so it holds in both
//! suites. Keep the code the same under both sets, or put that crate's GPU tests back
//! in the default pass by widening the `-E` in both callers; a switch no rendered path
//! reads says why with `// lint: allow(gpu-default-only) <why>`. A
//! `#[cfg(feature = "dev")]` only adds, and never counts; a `dev`-only *override* (a
//! value or a `path` set under `feature = "dev"`) changes what the GPU tests draw
//! without hiding anything, and is the reviewer's to catch.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;
use std::path::Path;

pub const RULE: &str = "gpu-default-only";

/// Every line of `path` (`test`, per line) whose predicate makes code exist only
/// without `dev`, when `path` is in one of `crates`: a `cfg!` on a production line,
/// a `cfg`/`cfg_attr` attribute starting on any line.
pub fn check_file(path: &str, source: &str, test: &[bool], crates: &[String]) -> Vec<Finding> {
    let Some(m) = crate::modules::crate_module(path) else {
        return Vec::new();
    };
    if !crates.iter().any(|c| c == m) || !crate::modules::module_path(path).starts_with("src/") {
        return Vec::new();
    }
    let lines = code_lines(source);
    (0..lines.len())
        .filter(|&i| {
            let test = test.get(i).copied().unwrap_or(false);
            (!test && value_splits(&lines[i])) || attribute_splits(&attribute_at(&lines, i))
        })
        .map(|i| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: format!(
                "code here exists only without `dev`, and no GPU test draws it: the default \
                 suite leaves the gpu group to the dev one (docs/testing.md, *Which suite runs \
                 the GPU tests*). Keep it the same under both feature sets, or add rusty-{m}'s \
                 GPU tests back to the default pass in `make test` and ci.yml together \
                 (`-E 'not group(gpu) | (group(gpu) & package(rusty-{m}))'`); a switch no \
                 rendered path reads, or a test that is not a GPU one, takes an inline allow \
                 saying why"
            ),
        })
        .collect()
}

/// The attribute that starts on line `i` (`#` first), its lines joined until its
/// brackets balance, as rustfmt wraps a long one; empty when none starts there.
fn attribute_at(lines: &[std::borrow::Cow<'_, str>], i: usize) -> String {
    if !lines[i].trim_start().starts_with('#') {
        return String::new();
    }
    let mut joined = String::new();
    let mut depth = 0i32;
    for line in &lines[i..] {
        joined.push_str(line);
        joined.push(' ');
        depth += line.matches('[').count() as i32 - line.matches(']').count() as i32;
        if depth <= 0 {
            break;
        }
    }
    joined
}

/// Does `code` hold a `cfg!` naming `dev`: a value that differs by feature set?
fn value_splits(code: &str) -> bool {
    predicates(code, "cfg!(").any(names_dev)
}

/// Does `attribute` (a joined `#[…]`) hold a `cfg`/`cfg_attr` whose `not(…)` names
/// `dev`?
fn attribute_splits(attribute: &str) -> bool {
    (predicates(attribute, "cfg(").chain(predicates(attribute, "cfg_attr(")))
        .any(|p| predicates(p, "not(").any(names_dev))
}

/// The balanced contents of every `open…)` in `code` (`open` ends in `(`), at a word
/// boundary.
fn predicates<'a>(code: &'a str, open: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    code.match_indices(open)
        .filter(move |(at, _)| {
            !code[..*at]
                .chars()
                .next_back()
                .is_some_and(crate::code::is_ident)
        })
        .map(move |(at, _)| {
            let rest = &code[at + open.len()..];
            let mut depth = 1;
            let end = rest
                .char_indices()
                .find(|&(_, c)| {
                    depth += match c {
                        '(' => 1,
                        ')' => -1,
                        _ => 0,
                    };
                    depth == 0
                })
                .map_or(rest.len(), |(i, _)| i);
            &rest[..end]
        })
}

/// Does `predicate` name `dev` so that it differs between the two suites? An
/// `any(…)` that also names `editor` does not: `editor` is a default feature, so it
/// holds in both and fails only in a ship build, which neither suite runs.
fn names_dev(predicate: &str) -> bool {
    let bare: String = predicate.chars().filter(|c| !c.is_whitespace()).collect();
    let both = bare.starts_with("any(") && bare.contains("feature=\"editor\"");
    bare.contains("feature=\"dev\"") && !both
}

/// The crates the rule holds: `render`, every crate its `dev` feature turns on
/// (`"rusty-<m>/dev"`, followed through each crate's own manifest), and every crate
/// one of whose `src/` test lines declares a `gpu_` test (`gpu_tests`).
pub fn scope(base: &Path, gpu_tests: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut crates = render_path(base);
    for m in gpu_tests {
        if !crates.contains(&m) {
            crates.push(m);
        }
    }
    crates
}

/// `render` and the crates its `dev` turns on, transitively, in the order found.
pub fn render_path(base: &Path) -> Vec<String> {
    let mut crates = vec!["render".to_string()];
    let mut i = 0;
    while i < crates.len() {
        let manifest = base.join("crates").join(&crates[i]).join("Cargo.toml");
        for m in dev_features(&std::fs::read_to_string(manifest).unwrap_or_default()) {
            if !crates.contains(&m) {
                crates.push(m);
            }
        }
        i += 1;
    }
    crates
}

/// The `rusty-<m>` crates a manifest's `dev` feature turns on (`"rusty-<m>/dev"`,
/// `"rusty-<m>?/dev"`), its array possibly wrapped over lines.
fn dev_features(manifest: &str) -> Vec<String> {
    let Some(at) = manifest
        .lines()
        .position(|l| l.trim_start().starts_with("dev ") || l.trim_start().starts_with("dev="))
    else {
        return Vec::new();
    };
    let mut array = String::new();
    for line in manifest.lines().skip(at) {
        array.push_str(line);
        if line.contains(']') {
            break;
        }
    }
    array
        .split('"')
        .filter_map(|s| s.strip_prefix("rusty-")?.strip_suffix("/dev"))
        .map(|s| s.trim_end_matches('?').to_string())
        .collect()
}

/// The crate of `path` when one of its `src/` test lines declares a `gpu_` test.
fn declares_gpu_test(path: &str, source: &str, test: &[bool]) -> Option<String> {
    let m = crate::modules::crate_module(path)?;
    let declares = code_lines(source)
        .iter()
        .enumerate()
        .any(|(i, code)| test.get(i).copied().unwrap_or(false) && code.contains("fn gpu_"));
    (declares && crate::modules::module_path(path).starts_with("src/")).then(|| m.to_string())
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    let gpu = scan
        .files
        .iter()
        .filter_map(|p| declares_gpu_test(p, scan.source(p), scan.test_lines(p)));
    let crates = scope(&scan.base, gpu);
    scan.each_file(|p, s| check_file(p, s, scan.test_lines(p), &crates))
}

#[cfg(test)]
#[path = "gpu_default_only_tests.rs"]
mod tests;
