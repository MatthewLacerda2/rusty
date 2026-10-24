//! `lua-path-slash`: a path handed to Lua is `/`-spelled (#1390).
//!
//! Scene asset paths are `/` on every OS, so a path an API function returns to a
//! script (and the script then writes into a scene) must be too. A bare
//! `to_string_lossy()` keeps the native `\` on Windows, and the scene it ends up in
//! stops resolving on macOS and Linux. In `src/api/` and `src/dev/lua_surface/`
//! (the code that marshals values to Lua), a `to_string_lossy()` outside test code
//! is a finding: spell the path with `crate::core::paths::to_slash`. A call on
//! something that is not a path (a Lua string) says so with
//! `// lint: allow(lua-path-slash) <why>`.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "lua-path-slash";
const SCOPES: [&str; 2] = ["src/api/", "src/dev/lua_surface/"];

/// Every non-test line of a Lua-facing source that calls `to_string_lossy()`.
pub fn check_file(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    if !SCOPES.iter().any(|s| path.starts_with(s)) {
        return Vec::new();
    }
    code_lines(source)
        .iter()
        .enumerate()
        .filter(|(i, code)| !test[*i] && code.contains(".to_string_lossy()"))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "a path handed to Lua is `/`-spelled; use `crate::core::paths::to_slash`"
                .to_string(),
        })
        .collect()
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check_file(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "lua_path_slash_tests.rs"]
mod tests;
