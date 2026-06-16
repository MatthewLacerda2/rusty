//! tools/lint — project size gate (the custom part clippy can't do).
//!
//! Counts per-file lines and fails on any file over its cap. Function-length and
//! code-smell checks are clippy's job (see clippy.toml); this binary only enforces
//! the per-file / per-test-file size rules. Dependency-free, std-only.
//!
//! Usage:
//!   cargo run --manifest-path tools/lint/Cargo.toml            # scan src/, minus baseline
//!   cargo run --manifest-path tools/lint/Cargo.toml -- a.rs b.rs   # check just these files
//!
//! Output: result to stdout and `.lint/report.txt`; exit code 1 on any violation.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

mod components;
mod determinism;

const MAX_FILE_LINES: usize = 300;
/// Test/fixture files get a looser cap than source: Rust test files legitimately
/// bundle several `#[test]` fns plus fixtures, and over-splitting them hurts
/// readability more than it helps.
const MAX_TEST_FILE_LINES: usize = 150;
const BASELINE: &str = "tools/lint/baseline.txt";
const REPORT: &str = ".lint/report.txt";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // `--determinism` runs the sim-purity guard instead of the size gate: it fails
    // if wall-clock / unseeded-RNG calls leak into the deterministic sim modules.
    if args.iter().any(|a| a == "--determinism") {
        determinism::run();
        return;
    }

    // `--components` runs the 4-axis component-completeness gate (#81): every
    // built-in component must have an Entity field, an Add Component entry, an
    // inspector card, and an API namespace, or be grandfathered in the baseline.
    if args.iter().any(|a| a == "--components") {
        components::run();
        return;
    }

    let files = if args.is_empty() {
        scan_dir(Path::new("src"))
    } else {
        args.iter().map(PathBuf::from).collect()
    };
    let baseline = load_baseline();

    let mut violations = Vec::new();
    for path in files {
        if !is_rust(&path) || is_baselined(&path, &baseline) {
            continue;
        }
        if let Some(v) = check_file(&path) {
            violations.push(v);
        }
    }
    report(&violations);
    if violations.is_empty() {
        println!("lint: ok");
    } else {
        exit(1);
    }
}

fn scan_dir(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(dir, &mut out);
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if is_rust(&path) {
            out.push(path);
        }
    }
}

fn is_rust(path: &Path) -> bool {
    path.extension().map_or(false, |e| e == "rs")
}

/// Test and fixture files get the tighter cap.
fn limit_for(path: &Path) -> usize {
    let p = normalize(path);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let in_test_dir = p.split('/').any(|seg| seg == "tests" || seg == "fixtures");
    let is_test = in_test_dir || name.ends_with("_test.rs") || name.starts_with("test_");
    if is_test {
        MAX_TEST_FILE_LINES
    } else {
        MAX_FILE_LINES
    }
}

fn check_file(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let lines = content.lines().count();
    let limit = limit_for(path);
    if lines > limit {
        Some(format!(
            "FILE_TOO_LONG {} {}/{}",
            normalize(path),
            lines,
            limit
        ))
    } else {
        None
    }
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches("./")
        .replace('\\', "/")
}

fn load_baseline() -> Vec<String> {
    fs::read_to_string(BASELINE)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

fn is_baselined(path: &Path, baseline: &[String]) -> bool {
    let n = normalize(path);
    baseline.iter().any(|b| *b == n)
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
            "\n{} file(s) over the size cap. Split them meaningfully (see docs/linting.md).",
            violations.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn normal_files_get_the_default_cap() {
        assert_eq!(limit_for(Path::new("src/render/mod.rs")), MAX_FILE_LINES);
    }

    #[test]
    fn test_and_fixture_files_get_the_tight_cap() {
        assert_eq!(limit_for(Path::new("tests/foo.rs")), MAX_TEST_FILE_LINES);
        assert_eq!(limit_for(Path::new("src/bar_test.rs")), MAX_TEST_FILE_LINES);
        assert_eq!(
            limit_for(Path::new("src/fixtures/baz.rs")),
            MAX_TEST_FILE_LINES
        );
    }

    #[test]
    fn normalize_strips_leading_dot_slash() {
        assert_eq!(normalize(Path::new("./src/main.rs")), "src/main.rs");
    }
}
