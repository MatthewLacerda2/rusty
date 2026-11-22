use std::path::Path;

pub(super) const MAX_FILE_LINES: usize = 300;
pub(super) const MAX_TEST_FILE_LINES: usize = 150;
pub(super) fn limit_for(base: &Path, path: &Path) -> usize {
    let p = super::normalize(super::relative(base, path));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
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

/// The one definition the size gate and `dir-size` share.
pub(crate) fn is_test_file(path: &str) -> bool {
    let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
    dir.split('/')
        .any(|seg| seg == "tests" || seg == "fixtures")
        || name == "tests.rs"
        || name.ends_with("_test.rs")
        || name.ends_with("_tests.rs")
        || name.starts_with("test_")
}

/// The suffix alone is not enough: a `_tests.rs` earns the source cap only with its source beside it.
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
