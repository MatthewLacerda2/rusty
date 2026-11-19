//! `fold-names`: every source the dead-pub fold rewrites names its sibling
//! crates in a way the fold can fold.
//!
//! `make dead-pub` (`dead_pub`) compiles a copy of the workspace folded into one
//! crate, so each member's, bin's and integration test's name for a sibling crate
//! or the lib is rewritten to a `crate::` path. Compiling that copy takes a whole
//! engine build, so it runs weekly, as a signal; a spelling it cannot rewrite
//! would leave the signal red and silent until then. This rule reads the same
//! files with the fold's own tokenizer (`dead_pub::names`), so it flags exactly
//! what the rewrite leaves: a `pub use` of a whole crate, and the lib named by
//! itself (`use rusty;`). Every other form folds: a path (`rusty_<m>::…`), and a
//! bare name in a `use`, a `use` group, an `extern crate`, an attribute or a macro
//! argument. Nothing excuses a finding (`Allow::Never`): an allow would not make
//! the folded copy compile, so the fix is the line, or teaching the fold the form.

use super::finding::Finding;
use super::scan::Scan;
use crate::dead_pub::{fold, workspace};
use std::fs;

pub const RULE: &str = "fold-names";

/// The rule over the scanned files the fold rewrites: each member's `src/`,
/// each `[[bin]]` and its module directory, and `tests/`. The root's own lib is
/// not one: the fold mounts its re-exports instead (`workspace::mount`).
pub fn run(scan: &Scan) -> Vec<Finding> {
    let read = |p: &str| fs::read_to_string(scan.root().join(p)).unwrap_or_default();
    let manifest = read("Cargo.toml");
    let Some(lib) = workspace::package_name(&manifest).map(|n| n.replace('-', "_")) else {
        return Vec::new();
    };
    let members = workspace::members(scan.root(), &read("src/lib.rs")).unwrap_or_default();
    let names: Vec<(String, String)> = members
        .iter()
        .map(|m| (m.ident.clone(), m.module.clone()))
        .collect();
    let mut folded: Vec<String> = members.iter().map(|m| format!("{}/src/", m.dir)).collect();
    folded.push("tests/".to_string());
    for bin in fold::bins(&manifest) {
        folded.push(bin.path.trim_end_matches(".rs").to_string() + "/");
        folded.push(bin.path);
    }
    scan.each_file(|path, source| {
        if !folded
            .iter()
            .any(|f| path == f || (f.ends_with('/') && path.starts_with(f.as_str())))
        {
            return Vec::new();
        }
        check(path, source, &lib, &names)
    })
}

/// Every name in `source` the fold must leave, at its line.
pub fn check(path: &str, source: &str, lib: &str, members: &[(String, String)]) -> Vec<Finding> {
    fold::unfoldable(source, "m", lib, members)
        .into_iter()
        .map(|(line, left)| Finding {
            path: path.to_string(),
            line,
            rule: RULE,
            message: format!(
                "the dead-pub fold cannot rewrite `{}` here, so `make dead-pub` stops \
                 compiling: {}. Every other spelling folds: a path, or a bare name in \
                 `use … as …;`, a `use` group, `extern crate`, an attribute or a macro \
                 argument (tools/lint/src/dead_pub/names.rs)",
                left.name, left.why
            ),
        })
        .collect()
}

#[cfg(test)]
#[path = "fold_names_tests.rs"]
mod tests;
