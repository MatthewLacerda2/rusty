//! The size gate: per-file caps over [`SIZE_ROOTS`] (or an explicit file list).
//! Result to stdout and `.lint/report.txt`.
//!
//! It measures Rust (`.rs`, every line) and, by lines of code (comments, docstrings
//! and blank lines free), Python (`.py`, [`python::code_lines`]), the engine's Lua
//! (`.lua`, [`lua::code_lines`]) and its shaders (`.wgsl`, [`wgsl::code_lines`]).
//! The merge path in `.github/scripts/` is the most agent-edited code outside
//! `src/`, and until #926 the gate could not see it: `merge-queue.py` had reached
//! 1,074 lines unseen. The shaders and the engine's script components had reached
//! twice the cap the same way before #1015.
//!
//! The full scan measures what can be committed ([`listing`], #1373): a file the
//! engine seeds into a gitignored folder is never pushed, so it never counts.
//!
//! **Nothing is grandfathered** (#926). A file over its cap is split, never excused;
//! the burn-down list this gate once read emptied and was retired.

use crate::gates::Verdict;
use std::fs;
use std::path::{Path, PathBuf};

mod listing;
mod lua;
mod python;
#[cfg(test)]
mod tests;
mod wgsl;

const MAX_FILE_LINES: usize = 300;
/// Test/fixture files get a looser cap than source: Rust test files legitimately
/// bundle several `#[test]` fns plus fixtures, and over-splitting them hurts
/// readability more than it helps.
const MAX_TEST_FILE_LINES: usize = 150;
/// The directories the `.rs` scans walk: the engine (`src/` and the member crates
/// under `crates/`), its integration suite, the fuzz
/// targets (#535), and the dev-tool crates under `tools/` (#541). The rules gate
/// walks these too.
pub(crate) const SCAN_ROOTS: &[&str] = &["src", "crates", "tests", "fuzz", "tools"];
/// What the size gate walks: [`SCAN_ROOTS`] plus `.github`, where the merge and
/// mutation scripts live (#926), and `engine`, the shipped shaders and script
/// components (#1015). `assets/` is not one: it is gitignored, the open project's
/// seeded and baked copies, so nothing in it is ever pushed; nor is a gitignored
/// path inside a root ([`full_scan`]).
const SIZE_ROOTS: &[&str] = &[
    "src", "crates", "tests", "fuzz", "tools", ".github", "engine",
];

/// The languages the size gate measures.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Lang {
    Rust,
    Python,
    Lua,
    Wgsl,
}

impl Lang {
    fn of(path: &Path) -> Option<Lang> {
        match path.extension()?.to_str()? {
            "rs" => Some(Lang::Rust),
            "py" => Some(Lang::Python),
            "lua" => Some(Lang::Lua),
            "wgsl" => Some(Lang::Wgsl),
            _ => None,
        }
    }

    /// The length the cap is held against: every line of Rust (#926 leaves Rust's
    /// counting as it was), the lines of code of the rest.
    fn measure(self, content: &str) -> usize {
        match self {
            Lang::Rust => content.lines().count(),
            Lang::Python => python::code_lines(content),
            Lang::Lua => lua::code_lines(content),
            Lang::Wgsl => wgsl::code_lines(content),
        }
    }
}

/// The gate over the repository at `base`: `args` (explicit files, relative to it)
/// or, when empty, every size root; and the agent docs among them. Each file is
/// judged and named by its path relative to `base`. Where git can't list the tree,
/// the body says so.
pub fn verdict(base: &Path, args: &[String]) -> Verdict {
    let mut notes = Vec::new();
    let files: Vec<PathBuf> = if args.is_empty() {
        let (files, note) = full_scan(base);
        notes.extend(note);
        files
    } else {
        args.iter().map(|a| base.join(a)).collect()
    };

    let violations: Vec<String> = files.iter().filter_map(|p| check_file(base, p)).collect();
    let (docs, note) = crate::agent_docs::gate(base, args);
    notes.extend(note);
    let (over, unshaped) = (violations.len(), docs.len());
    let findings: Vec<String> = violations.into_iter().chain(docs).collect();
    let mut verdict = Verdict::listed("lint", findings, |_| {
        let mut advice = String::new();
        if over > 0 {
            advice.push_str(&format!(
                "\n{over} file(s) over the size cap. Split them by responsibility; nothing \
                 is grandfathered (see docs/linting.md).\n"
            ));
        }
        if unshaped > 0 {
            advice.push_str(&format!(
                "\n{unshaped} agent-doc finding(s) (`CLAUDE.md`, skill files and \
                 `.claude/agents/` definitions; docs/linting.md, *Agent docs*).\n"
            ));
        }
        advice
    });
    for note in notes {
        verdict.body.push_str(&note);
        verdict.body.push('\n');
    }
    verdict
}

/// Every measured file under [`SIZE_ROOTS`] in the repository at `base`: the
/// committable ones git lists, or, where git can't list them (a tarball with no
/// `.git`), every file on disk, build output aside, with a note saying so.
fn full_scan(base: &Path) -> (Vec<PathBuf>, Option<String>) {
    match listing::committable(base, SIZE_ROOTS) {
        Ok(files) => {
            let files = files.into_iter().filter(|p| Lang::of(p).is_some());
            (files.collect(), None)
        }
        Err(why) => {
            let files = SIZE_ROOTS
                .iter()
                .flat_map(|root| measured_in(&base.join(root)))
                .collect();
            (files, Some(fallback_note(&why)))
        }
    }
}

/// The note for the disk walk: one line saying so, then git's own message
/// (several lines for "dubious ownership") on lines of its own.
fn fallback_note(why: &str) -> String {
    format!(
        "lint: git can't list the committable files here; measuring every file on disk, \
         gitignored ones included\ngit said:\n{why}"
    )
}

/// Every `.rs` file under `dir`, build output aside: what the rules gate scans.
pub(crate) fn scan_dir(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(dir, &mut out, &|p| Lang::of(p) == Some(Lang::Rust));
    out
}

/// Every `.wgsl` shader under `dir`, build output aside: what the shader rules read.
pub(crate) fn shaders_in(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(dir, &mut out, &|p| Lang::of(p) == Some(Lang::Wgsl));
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
fn limit_for(base: &Path, path: &Path) -> usize {
    let p = normalize(relative(base, path));
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

/// The violation line for `path` (on disk, under `base`), or `None` when it fits
/// (or is not measured).
fn check_file(base: &Path, path: &Path) -> Option<String> {
    let lang = Lang::of(path)?;
    let content = fs::read_to_string(path).ok()?;
    let lines = lang.measure(&content);
    let limit = limit_for(base, path);
    if lines <= limit {
        return None;
    }
    // A lines-of-code count is not `wc -l`'s, so the line says which it is.
    let unit = if lang != Lang::Rust {
        " (lines of code)"
    } else {
        ""
    };
    Some(format!(
        "FILE_TOO_LONG {} {lines}/{limit}{unit}",
        normalize(relative(base, path))
    ))
}

/// `path` with `base` stripped: what the repository at `base` calls it.
fn relative<'a>(base: &Path, path: &'a Path) -> &'a Path {
    path.strip_prefix(base).unwrap_or(path)
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy().trim_start_matches("./").to_string()
}
