//! Which cap a file is held to: the source cap, or the tighter one for a test or
//! fixture file.

use std::path::Path;

pub(super) const MAX_FILE_LINES: usize = 300;
/// Test/fixture files get a looser cap than source: Rust test files legitimately
/// bundle several `#[test]` fns plus fixtures, and over-splitting them hurts
/// readability more than it helps.
pub(super) const MAX_TEST_FILE_LINES: usize = 150;
/// The cap a file is measured against.
///
/// Standalone test/fixture files (`tests/`, `fixtures/`, `*_test.rs`, a `*_tests.rs`
/// with no source beside it, `test_*`) get
/// the tighter [`MAX_TEST_FILE_LINES`] cap, because over-splitting a bundle of
/// `#[test]`s hurts readability more than it helps. The same holds for Python:
/// everything under a `tests/` directory is a test file.
///
/// A `<name>_tests.rs` **sibling** is the exception: it is the dedicated, single
/// test home for `<name>.rs` next to it, so source + sibling read as one logical
/// unit. Splitting a source file's tests between an inline `#[cfg(test)] mod tests`
/// AND such a sibling, purely to fit the tight cap, fragments that unit — so the
/// sibling carries the source cap ([`MAX_FILE_LINES`]), letting all of one source's
/// tests live in one place. (See issue #211.) Only a real sibling earns it (#960).
///
/// `path` is the file on disk; it is judged by its part under `base`, so a
/// `tests/` above the repository gives nothing the test cap.
pub(super) fn limit_for(base: &Path, path: &Path) -> usize {
    let p = super::normalize(super::relative(base, path));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    // `tests.rs` is its module directory's own tests sibling, but only outside a
    // test folder: `tests/x/tests.rs` is a standalone test file like its neighbours.
    let in_test_dir = p.rsplit_once('/').is_some_and(|(dir, _)| {
        dir.split('/')
            .any(|seg| seg == "tests" || seg == "fixtures")
    });
    if is_tests_sibling(path) || (name == "tests.rs" && !in_test_dir) {
        return MAX_FILE_LINES;
    }
    if is_test_file(&p) {
        MAX_TEST_FILE_LINES
    } else {
        MAX_FILE_LINES
    }
}

/// Is `path` (`/`-separated) a test or fixture file by its name or folder: under a
/// `tests/` or `fixtures/` directory, `tests.rs`, `*_test.rs`, `*_tests.rs` or
/// `test_*`? The one definition the size gate and `dir-size` share (#1367).
pub(crate) fn is_test_file(path: &str) -> bool {
    let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
    dir.split('/')
        .any(|seg| seg == "tests" || seg == "fixtures")
        || name == "tests.rs"
        || name.ends_with("_test.rs")
        || name.ends_with("_tests.rs")
        || name.starts_with("test_")
}

/// Is `path` a `<x>_tests.rs` file with its source beside it — `<x>.rs`, or a `<x>/`
/// module directory (which holds `<x>/mod.rs`)? Such a sibling is part of that
/// source's logical unit (the source cap), not an over-split standalone test file.
///
/// The suffix alone is not enough (#960): it once granted the source cap to any
/// `_tests.rs`, and ten files with no source beside them had grown past the tight
/// cap on that free pass. The check reads the disk beside `path`, so it holds
/// whether the lint scans the roots or checks an explicit file list.
fn is_tests_sibling(path: &Path) -> bool {
    let Some(stem) = path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix("_tests.rs"))
        .filter(|stem| !stem.is_empty())
    else {
        return false;
    };
    let dir = path.parent().unwrap_or(Path::new(""));
    dir.join(format!("{stem}.rs")).is_file() || dir.join(stem).is_dir()
}
