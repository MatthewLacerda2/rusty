//! `gpu-default-only`: no render-path code exists only without `dev`.
//!
//! The default-feature test suite leaves the `gpu` test group to the `dev` suite
//! (`make test`'s `DEFAULT_SUITE`, CI's "Test (engine, default)" step; docs/testing.md,
//! *Which suite runs the GPU tests*). That is sound only while `dev` adds to the
//! default build and never takes code away from what is drawn: code that compiles
//! only without `dev` is drawn by no GPU test any more. So in a crate on the render
//! path (`rusty-render` and every crate whose `dev` its own `dev` turns on, read from
//! `crates/render/Cargo.toml`), a production line holding a `cfg!(…)` that names
//! `feature = "dev"` (a value that differs by feature set) or a `cfg(…)` /
//! `cfg_attr(…)` attribute whose `not(…)` names it is a finding (`any(dev, editor)`
//! is not: `editor` is a default feature, so it holds in both suites). Keep the code the
//! same under both sets, or put that crate's GPU tests back in the default pass by
//! widening the `-E` in both callers; a switch no rendered path reads says why with
//! `// lint: allow(gpu-default-only) <why>`. A `#[cfg(feature = "dev")]` only adds,
//! and never counts.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;
use std::path::Path;

pub const RULE: &str = "gpu-default-only";

/// Every production line of `path` (`test`, per line) whose predicate makes code
/// exist only without `dev`, when `path` is in one of `crates`.
pub fn check_file(path: &str, source: &str, test: &[bool], crates: &[String]) -> Vec<Finding> {
    let Some(m) = crate::modules::crate_module(path) else {
        return Vec::new();
    };
    if !crates.iter().any(|c| c == m) || !crate::modules::module_path(path).starts_with("src/") {
        return Vec::new();
    }
    code_lines(source)
        .iter()
        .enumerate()
        .filter(|(i, code)| !test.get(*i).copied().unwrap_or(false) && splits(code))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: format!(
                "code here exists only without `dev`, and no GPU test draws it: the default \
                 suite leaves the gpu group to the dev one (docs/testing.md, *Which suite runs \
                 the GPU tests*). Keep it the same under both feature sets, or add rusty-{m}'s \
                 GPU tests back to the default pass in `make test` and ci.yml together \
                 (`-E 'not group(gpu) | (group(gpu) & package(rusty-{m}))'`); a switch no \
                 rendered path reads takes an inline allow saying why"
            ),
        })
        .collect()
}

/// Does `code` hold a `cfg!` naming `dev`, or a `cfg`/`cfg_attr` attribute whose
/// `not(…)` names it?
fn splits(code: &str) -> bool {
    let value = predicates(code, "cfg!(").any(names_dev);
    let attribute = code.trim_start().starts_with('#')
        && (predicates(code, "cfg(").chain(predicates(code, "cfg_attr(")))
            .any(|p| predicates(p, "not(").any(names_dev));
    value || attribute
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

/// `render` and every crate whose `dev` feature `crates/render/Cargo.toml`'s own
/// `dev` turns on (`"rusty-<m>/dev"`).
pub fn render_path(base: &Path) -> Vec<String> {
    let manifest =
        std::fs::read_to_string(base.join("crates/render/Cargo.toml")).unwrap_or_default();
    let dev = manifest
        .lines()
        .find(|l| l.trim_start().starts_with("dev ") || l.trim_start().starts_with("dev="))
        .unwrap_or_default();
    let mut crates = vec!["render".to_string()];
    crates.extend(
        dev.split('"')
            .filter_map(|s| s.strip_prefix("rusty-")?.strip_suffix("/dev"))
            .map(str::to_string),
    );
    crates
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    let crates = render_path(&scan.base);
    scan.each_file(|p, s| check_file(p, s, scan.test_lines(p), &crates))
}

#[cfg(test)]
#[path = "gpu_default_only_tests.rs"]
mod tests;
