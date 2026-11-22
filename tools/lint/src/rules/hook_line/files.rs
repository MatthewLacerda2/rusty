use crate::modules::crate_module;

/// A file's crate, `""` for the root package.
pub fn package(path: &str) -> Option<&str> {
    match crate_module(path) {
        Some(m) => Some(m),
        None if path.starts_with("src/") || path.starts_with("tests/") => Some(""),
        None => None,
    }
}

/// A file only tests or tools compile: under `tests/`, `benches/` or `examples/`, or a
/// `tests.rs` / `*_tests.rs`. Not `size::is_test_file`, which also takes a `test_*.rs`:
/// here that name is a hook module's (`render::test_gpu`), which is claimed and called.
pub fn test_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.split('/')
        .any(|seg| matches!(seg, "tests" | "benches" | "examples"))
        || name == "tests.rs"
        || name.ends_with("_tests.rs")
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
