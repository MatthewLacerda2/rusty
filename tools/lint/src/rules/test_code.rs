//! Which lines of a source file are test code: everything under `tests/`, a
//! `tests.rs` or `*_tests.rs` file, and a file's lines from its first
//! `#[cfg(test)]` on (the inline `mod tests` shape). Only `src/` and `tests/` hold
//! the engine's tests; other roots never count.

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
    source
        .lines()
        .position(|l| l.trim_start().starts_with("#[cfg(test)]"))
}
