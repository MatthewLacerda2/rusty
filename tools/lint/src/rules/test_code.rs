//! Which lines of a source file are test code: everything under `tests/`, a
//! `tests.rs` or `*_tests.rs` file, and a file's lines from its first
//! `#[cfg(test)]` or `#[cfg(all(…, test, …))]` on, however the attribute is wrapped
//! (the inline `mod tests` shape). `any(test, …)` is not test-only (it also compiles
//! outside tests), so it stays production code. Only `src/` and `tests/` hold the
//! engine's tests; other roots never count.

/// Is the whole of `path` test code?
fn test_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.starts_with("tests/") || name == "tests.rs" || name.ends_with("_tests.rs")
}

/// The 0-based index of the first test line of `source`, or `None` when the file
/// holds no test code.
pub fn first_line(path: &str, source: &str) -> Option<usize> {
    if !(path.starts_with("src/") || path.starts_with("tests/")) {
        return None;
    }
    if test_file(path) {
        return Some(0);
    }
    let lines: Vec<&str> = source.lines().collect();
    (0..lines.len()).find(|&i| is_test_gate(&attribute(&lines[i..])))
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
