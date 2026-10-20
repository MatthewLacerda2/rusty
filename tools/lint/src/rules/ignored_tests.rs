use super::{check, RULE};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// A throwaway git repo whose `.gitignore` holds the unanchored `out/`.
fn repo(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lint_ignored_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir");
    let ok = Command::new("git")
        .args(["init", "-q"])
        .current_dir(&dir)
        .status()
        .expect("git");
    assert!(ok.success());
    fs::write(dir.join(".gitignore"), "out/\n").expect("gitignore");
    dir
}

#[test]
fn flags_a_scanned_file_a_pattern_excludes() {
    let dir = repo("flag");
    let files = ["src/out/a.rs".to_string(), "src/a.rs".to_string()];
    let found = check(dir.to_str().expect("utf-8"), &files);
    let paths: Vec<_> = found.iter().map(|f| (f.path.as_str(), f.rule)).collect();
    assert_eq!(paths, [("src/out/a.rs", RULE)]);
    fs::remove_dir_all(dir).ok();
}

#[test]
fn passes_when_nothing_is_excluded() {
    let dir = repo("pass");
    let files = ["src/a.rs".to_string(), "tests/main.rs".to_string()];
    assert!(check(dir.to_str().expect("utf-8"), &files).is_empty());
    fs::remove_dir_all(dir).ok();
}

#[test]
fn a_git_failure_is_a_finding_not_a_pass() {
    let found = check("/nonexistent/lint_ignored", &["a.rs".to_string()]);
    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains("could not run"), "{}", found[0]);
}
