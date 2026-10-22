//! The `tests/` source-shape locks (#980): three `docs/testing.md` rules that keep
//! a test from silently not running, or from depending on the machine it runs on.
//!
//! - **`test-inner-cfg`** — a dev-only `tests/` file is gated where it is declared,
//!   `#[cfg(feature = "dev")] mod …;`. An inner `#![cfg(…)]` compiles the file away
//!   while its `mod` line still reads as "this runs".
//! - **`test-user-workspace`** — a test never reads a developer's game project
//!   (#782): `Harness::in_user_workspace` and `scenario::run(` are the CLI's; a test
//!   runs a scenario with `scenario::run_isolated`.
//! - **`test-gpu-entry`** — test code asks for a GPU only through
//!   `crate::render::test_gpu::headless_or_skip`, the one place that decides what
//!   "no GPU here" means; a direct `Renderer::new_headless` drifts from it.
//!
//! Test code is what `test_code.rs` says it is. Comment lines never count, so a doc
//! may name the calls.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const INNER_CFG: &str = "test-inner-cfg";
pub const USER_WORKSPACE: &str = "test-user-workspace";
pub const GPU_ENTRY: &str = "test-gpu-entry";

/// The one file that may call `new_headless` from test code.
const GPU_HELPER: &str = "src/render/test_gpu.rs";

/// Every break of the three locks in the test code of `path` (`test`, per line).
pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let mut found = Vec::new();
    let mut flag = |line: usize, rule: &'static str, message: &str| {
        found.push(Finding {
            path: path.to_string(),
            line,
            rule,
            message: message.to_string(),
        });
    };
    for (i, line) in code_lines(source)
        .iter()
        .enumerate()
        .filter(|(i, _)| test[*i])
    {
        let code = line.trim_start();
        if path.starts_with("tests/") && code.starts_with("#![cfg(") {
            flag(
                i + 1,
                INNER_CFG,
                "gate the file at its `mod` line (`#[cfg(feature = \"dev\")] mod …;`), \
                 not with an inner `#![cfg]`",
            );
        }
        if code.contains("in_user_workspace") || code.contains("scenario::run(") {
            flag(
                i + 1,
                USER_WORKSPACE,
                "a test never reads the developer's project; use `scenario::run_isolated`",
            );
        }
        if path != GPU_HELPER && code.contains("new_headless(") {
            flag(
                i + 1,
                GPU_ENTRY,
                "ask for a GPU through `crate::render::test_gpu::headless_or_skip`",
            );
        }
    }
    found
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "test_shape_tests.rs"]
mod tests;
