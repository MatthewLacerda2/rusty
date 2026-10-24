use super::{check, RULE};
use crate::repo::scaffold::Repo;

/// A throwaway git repo whose `.gitignore` holds the unanchored `out/`.
fn repo(name: &str) -> Repo {
    let repo = Repo::new(&format!("ignored-{name}"), true);
    repo.file(".gitignore", "out/\n");
    repo
}

#[test]
fn flags_a_scanned_file_a_pattern_excludes() {
    let dir = repo("flag");
    let files = ["src/out/a.rs".to_string(), "src/a.rs".to_string()];
    let found = check(dir.0.to_str().expect("utf-8"), &files);
    let paths: Vec<_> = found.iter().map(|f| (f.path.as_str(), f.rule)).collect();
    assert_eq!(paths, [("src/out/a.rs", RULE)]);
}

#[test]
fn passes_when_nothing_is_excluded() {
    let dir = repo("pass");
    let files = ["src/a.rs".to_string(), "tests/main.rs".to_string()];
    assert!(check(dir.0.to_str().expect("utf-8"), &files).is_empty());
}

#[test]
fn a_git_failure_is_a_finding_not_a_pass() {
    let found = check("/nonexistent/lint_ignored", &["a.rs".to_string()]);
    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains("could not run"), "{}", found[0]);
}
