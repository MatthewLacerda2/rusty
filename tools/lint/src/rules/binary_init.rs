//! `binary-init`: every engine binary starts with `rusty::init()`.
//!
//! `rusty::init` starts the logger and, in a `dev` build, checks that the `engine/`
//! folder on disk is the content the binary was built with
//! (`core::project::engine_content`). A binary that starts the logger itself
//! (`rusty::logging::init()`, which stays public for the library) builds and runs,
//! but never warns about a stale `engine/`, and its run reports read as an
//! unstamped build's. So the first code line inside `fn main` of every binary
//! root (`src/main.rs`, `src/bin/<name>.rs`, `src/bin/<name>/main.rs`) must call
//! `rusty::init()`; a binary that truly must not takes
//! `// lint: allow(binary-init) <why>` on its `fn main` line.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::bare_lines;

pub const RULE: &str = "binary-init";

/// Is `path` a binary root cargo builds from the root package's `src/`?
fn is_binary_root(path: &str) -> bool {
    if path == "src/main.rs" {
        return true;
    }
    let Some(rest) = path.strip_prefix("src/bin/") else {
        return false;
    };
    match rest.split_once('/') {
        None => rest.ends_with(".rs"),
        Some((_, file)) => file == "main.rs",
    }
}

/// A finding when `path`, a binary root, has a `fn main` whose first code line is
/// not a `rusty::init()` call.
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    if !is_binary_root(path) {
        return Vec::new();
    }
    let code = bare_lines(source);
    let Some(main) = code.iter().position(|l| l.contains("fn main(")) else {
        return Vec::new();
    };
    let first = code[main + 1..]
        .iter()
        .map(|l| l.trim())
        .find(|l| !l.is_empty());
    if first.is_some_and(|l| l.starts_with("rusty::init();")) {
        return Vec::new();
    }
    vec![Finding {
        path: path.to_string(),
        line: main + 1,
        rule: RULE,
        message: "a binary's `fn main` calls `rusty::init();` first thing: it starts the \
                  logger and, in a `dev` build, warns when engine/ is not the content this \
                  binary was built with; `rusty::logging::init()` alone skips that check and \
                  its run reports read as unstamped"
            .to_string(),
    }]
}

/// The rule over every scanned binary root.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "binary_init_tests.rs"]
mod tests;
