//! Which lines of a source file are test code: everything under `tests/`, a
//! `tests.rs` or `*_tests.rs` file, and the item under each `#[cfg(test)]` or
//! `#[cfg(all(…, test, …))]`, however the attribute is wrapped. A gated
//! `mod tests { … }` or `fn` counts to its closing brace, a gated `use` to its `;`,
//! a gated field or arm to its `,` (`layers::item_end`); the lines after the item
//! are production code again (#1113). A gated `mod x;` counts as its own line only
//! (#1120). `any(test, …)` is not test-only (it also compiles outside tests), so it
//! stays production code. Only `src/` and `tests/` hold the engine's tests; other
//! roots never count.

use crate::layers::item_end;

/// Is the whole of `path` test code?
fn test_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.starts_with("tests/") || name == "tests.rs" || name.ends_with("_tests.rs")
}

/// For each line of `source`, whether it is test code.
pub fn test_lines(path: &str, source: &str) -> Vec<bool> {
    let lines: Vec<&str> = source.lines().collect();
    if !(path.starts_with("src/") || path.starts_with("tests/")) {
        return vec![false; lines.len()];
    }
    if test_file(path) {
        return vec![true; lines.len()];
    }
    let mut test = vec![false; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        if is_test_gate(&attribute(&lines[i..])) {
            let end = item_end(&lines, i);
            test[i..=end].fill(true);
            i = end + 1;
        } else {
            i += 1;
        }
    }
    test
}

/// The attribute starting at `lines[0]`, wrapped lines joined and spaces dropped,
/// as far as its closing `)]` (rustfmt wraps a long `cfg` over several lines).
fn attribute(lines: &[&str]) -> String {
    let mut attr = String::new();
    for line in lines.iter().take(12) {
        attr.extend(line.split_whitespace());
        if attr.ends_with(")]") || !attr.starts_with("#[cfg(") {
            break;
        }
    }
    attr
}

/// Does `#[cfg(…)]` compile only under test: `test`, or an `all(…)` with a
/// test-only argument (`all(feature = "dev", test)`)? `any(…)` and `not(…)` do not.
fn is_test_gate(attr: &str) -> bool {
    attr.strip_prefix("#[cfg(")
        .and_then(|rest| rest.strip_suffix(")]"))
        .is_some_and(test_only)
}

fn test_only(predicate: &str) -> bool {
    if predicate == "test" {
        return true;
    }
    let Some(args) = predicate
        .strip_prefix("all(")
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        return false;
    };
    let (mut depth, mut start) = (0, 0);
    let mut parts = Vec::new();
    for (at, c) in args.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&args[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&args[start..]);
    parts.into_iter().any(test_only)
}
