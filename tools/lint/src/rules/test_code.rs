//! Which lines of a source file are test code: everything under `tests/`, a
//! `tests.rs` or `*_tests.rs` file, a file only a test-gated `mod x;` declares
//! (`Scan::test_lines`, #1120), and the item under each test gate
//! (`layers::is_test_gate`, the one definition the layering lint shares, #1122). A
//! gated `mod tests { … }` or `fn` counts to its closing brace, a gated `use` to
//! its `;`, a gated field or arm to its `,` (`layers::item_end`); the lines after
//! the item are production code again (#1113). Only `src/` and `tests/` hold the
//! engine's tests; other roots never count.

use crate::code::code_lines;
use crate::layers::{is_test_gate, item_end};

/// Is the whole of `path` test code by its name?
fn test_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.starts_with("tests/") || name == "tests.rs" || name.ends_with("_tests.rs")
}

/// For each line of `source`, whether it is test code. `declared_test_only` says
/// a test-gated `mod x;` declares the file, which makes all of it test code.
pub fn test_lines(path: &str, source: &str, declared_test_only: bool) -> Vec<bool> {
    let stripped = code_lines(source);
    let lines: Vec<&str> = stripped.iter().map(|l| l.as_ref()).collect();
    if !(path.starts_with("src/") || path.starts_with("tests/")) {
        return vec![false; lines.len()];
    }
    if declared_test_only || test_file(path) {
        return vec![true; lines.len()];
    }
    let mut test = vec![false; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        if is_test_gate(&lines[i..]) {
            let end = item_end(&lines, i);
            test[i..=end].fill(true);
            i = end + 1;
        } else {
            i += 1;
        }
    }
    test
}
