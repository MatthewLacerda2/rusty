//! `engine-spawn`: a test starts an engine binary only through `tests/spawn.rs`
//! (#974).
//!
//! `session` and `session-mcp` read provider keys at startup, so one started bare
//! by a test, on a machine with a key, could spend. `tests/spawn.rs`'s
//! `engine_bin!` sets `RUSTY_NO_PROVIDER_KEYS` on every spawn, so a code line
//! naming `CARGO_BIN_EXE_` anywhere else is a finding: that spawn would skip it.
//! `tools/` is not the engine's tests and is not scanned (`dead_pub` sets the
//! variable for the lib's own tests).

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "engine-spawn";

/// The one file that may name a binary's `CARGO_BIN_EXE_` variable.
pub const HELPER: &str = "tests/spawn.rs";

/// Every code line outside [`HELPER`] and `tools/` that names `CARGO_BIN_EXE_`.
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    if path == HELPER || path.starts_with("tools/") {
        return Vec::new();
    }
    code_lines(source)
        .iter()
        .enumerate()
        .filter(|(_, code)| code.contains("CARGO_BIN_EXE_"))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: format!(
                "start an engine binary with `crate::spawn::engine_bin!(\"<bin>\")` ({HELPER}), \
                 which keeps provider keys out of it"
            ),
        })
        .collect()
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "engine_spawn_tests.rs"]
mod tests;
