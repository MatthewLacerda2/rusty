//! The size gate: per-file caps over [`SIZE_ROOTS`] (or an explicit file list).
//! Result to stdout and `.lint/report.txt`.
//!
//! It measures Rust (`.rs`, every line) and Python (`.py`, lines of code: docstrings,
//! comments and blank lines are free, [`python::code_lines`]). The merge path in
//! `.github/scripts/` is the most agent-edited code outside `src/`, and until #926
//! the gate could not see it: `merge-queue.py` had reached 1,074 lines unseen.
//!
//! **Nothing is grandfathered** (#926). A file over its cap is split, never excused;
//! the burn-down list this gate once read emptied and was retired.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

mod python;
#[cfg(test)]
mod tests;

const MAX_FILE_LINES: usize = 300;
/// Test/fixture files get a looser cap than source: Rust test files legitimately
/// bundle several `#[test]` fns plus fixtures, and over-splitting them hurts
/// readability more than it helps.
const MAX_TEST_FILE_LINES: usize = 150;
/// The directories the `.rs` scans walk: the engine, its integration suite, the fuzz
/// targets (#535), and the dev-tool crates under `tools/` (#541). The rules gate
/// walks these too.
pub(crate) const SCAN_ROOTS: &[&str] = &["src", "tests", "fuzz", "tools"];
/// What the size gate walks: [`SCAN_ROOTS`] plus `.github`, where the merge and
/// mutation scripts live (#926).
const SIZE_ROOTS: &[&str] = &["src", "tests", "fuzz", "tools", ".github"];
const REPORT: &str = ".lint/report.txt";

/// The languages the size gate measures.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Lang {
    Rust,
    Python,
}

impl Lang {
    fn of(path: &Path) -> Option<Lang> {
        match path.extension()?.to_str()? {
            "rs" => Some(Lang::Rust),
            "py" => Some(Lang::Python),
            _ => None,
        }
    }

    /// The length the cap is held against: every line of Rust (#926 leaves Rust's
    /// counting as it was), Python's lines of code.
    fn measure(self, content: &str) -> usize {
        match self {
            Lang::Rust => content.lines().count(),
            Lang::Python => python::code_lines(content),
        }
    }
}

/// Entry point: check `args` (explicit files) or, when empty, every size root.
pub fn run(args: &[String]) {
    let files: Vec<PathBuf> = if args.is_empty() {
        SIZE_ROOTS
            .iter()
            .flat_map(|root| measured_in(Path::new(root)))
            .collect()
    } else {
        args.iter().map(PathBuf::from).collect()
    };

    let violations: Vec<String> = files.iter().filter_map(|p| check_file(p)).collect();
    report(&violations);
    if violations.is_empty() {
        println!("lint: ok");
    } else {
        exit(1);
    }
}

/// Every `.rs` file under `dir`, build output aside: what the rules gate scans.
pub(crate) fn scan_dir(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(dir, &mut out, &|p| Lang::of(p) == Some(Lang::Rust));
    out
}

/// Every file under `dir` the size gate measures, build output aside.
fn measured_in(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(dir, &mut out, &|p| Lang::of(p).is_some());
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>, wanted: &dyn Fn(&Path) -> bool) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Build output (e.g. `fuzz/target/` after `cargo fuzz`) holds generated
            // code, and `__pycache__` holds compiled Python.
            if path
                .file_name()
                .is_some_and(|n| n == "target" || n == "__pycache__")
            {
                continue;
            }
            walk(&path, out, wanted);
        } else if wanted(&path) {
            out.push(path);
        }
    }
}

/// The cap a file is measured against.
///
/// Standalone test/fixture files (`tests/`, `fixtures/`, `*_test.rs`, `test_*`) get
/// the tighter [`MAX_TEST_FILE_LINES`] cap, because over-splitting a bundle of
/// `#[test]`s hurts readability more than it helps. The same holds for Python:
/// everything under a `tests/` directory is a test file.
///
/// A `<name>_tests.rs` **sibling** is the exception: it is the dedicated, single
/// test home for `<name>.rs` next to it, so source + sibling read as one logical
/// unit. Splitting a source file's tests between an inline `#[cfg(test)] mod tests`
/// AND such a sibling, purely to fit the tight cap, fragments that unit — so the
/// sibling carries the source cap ([`MAX_FILE_LINES`]), letting all of one source's
/// tests live in one place. (See issue #211.)
fn limit_for(path: &Path) -> usize {
    let p = normalize(path);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if is_tests_sibling(&name) {
        return MAX_FILE_LINES;
    }
    let in_test_dir = p.split('/').any(|seg| seg == "tests" || seg == "fixtures");
    let is_test = in_test_dir || name.ends_with("_test.rs") || name.starts_with("test_");
    if is_test {
        MAX_TEST_FILE_LINES
    } else {
        MAX_FILE_LINES
    }
}

/// Is `name` a `<x>_tests.rs` file — the dedicated test home for a `<x>.rs` source
/// beside it? Such a sibling is part of that source's logical unit (the source cap),
/// not an over-split standalone test file. The naming convention (`_tests.rs`, the
/// plural sibling form, distinct from the standalone `_test.rs`) is the marker, so
/// the rule holds whether the lint scans `src/` or checks an explicit file list.
fn is_tests_sibling(name: &str) -> bool {
    name.ends_with("_tests.rs")
}

/// The violation line for `path`, or `None` when it fits (or is not measured).
fn check_file(path: &Path) -> Option<String> {
    let lang = Lang::of(path)?;
    let content = fs::read_to_string(path).ok()?;
    let lines = lang.measure(&content);
    let limit = limit_for(path);
    if lines <= limit {
        return None;
    }
    // Python's count is not `wc -l`'s, so the line says which it is.
    let unit = if lang == Lang::Python {
        " (lines of code)"
    } else {
        ""
    };
    Some(format!(
        "FILE_TOO_LONG {} {lines}/{limit}{unit}",
        normalize(path)
    ))
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches("./")
        .replace('\\', "/")
}

fn report(violations: &[String]) {
    let mut body = if violations.is_empty() {
        String::from("lint: ok\n")
    } else {
        String::from("lint: FAILED\n")
    };
    for v in violations {
        body.push_str(v);
        body.push('\n');
    }
    fs::create_dir_all(".lint").ok();
    fs::write(REPORT, &body).ok();
    if !violations.is_empty() {
        eprint!("{}", body);
        eprintln!(
            "\n{} file(s) over the size cap. Split them by responsibility; nothing is \
             grandfathered (see docs/linting.md).",
            violations.len()
        );
    }
}
