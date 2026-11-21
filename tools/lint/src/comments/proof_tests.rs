use super::verdict;
use crate::repo::scaffold::Repo;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Each scratch repository's own name: the tests run in parallel.
static NEXT: AtomicUsize = AtomicUsize::new(0);

const RUST: &str = "//! The module.\n#![doc = \"more\"]\n\n/// Adds.\n#[inline]\n\
    fn add(a: u32, /* the other */ b: u32) -> u32 {\n    let s = \"a // b\"; // trailing\n    \
    call(a, // why\n        b)\n}\n";

/// The findings of the proof over a repository whose `base` branch holds `files` and
/// whose `HEAD` commit then writes `edits` (a `None` body deletes the file).
pub(super) fn prove(
    files: &[(&str, &str)],
    edits: &[(&str, Option<&str>)],
) -> (Vec<String>, String) {
    let repo = Repo::new(
        &format!("comment-only-{}", NEXT.fetch_add(1, Ordering::Relaxed)),
        true,
    );
    let commit = |repo: &Repo| {
        repo.git(&["add", "-A"]);
        let who = [
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "x",
        ];
        repo.git(&who);
    };
    for (path, body) in files {
        repo.file(path, body);
    }
    commit(&repo);
    repo.git(&["branch", "base"]);
    for (path, body) in edits {
        match body {
            Some(body) => drop(repo.file(path, body)),
            None => std::fs::remove_file(repo.0.join(path)).expect("rm"),
        }
    }
    commit(&repo);
    let found = verdict(&repo.0, "base");
    (found.findings, found.body)
}

pub(super) fn passes(path: &str, old: &str, new: &str) -> bool {
    prove(&[(path, old)], &[(path, Some(new))]).0.is_empty()
}

#[test]
fn removing_comments_of_every_form_passes() {
    let cut =
        "#[inline]\nfn add(a: u32, b: u32) -> u32 {\n    let s = \"a // b\";\n    call(a, b)\n}\n";
    assert!(
        passes("src/a.rs", RUST, cut),
        "line, block and doc comments, a doc attribute, a re-joined line"
    );
    assert!(passes(
        "e/s.lua",
        "-- a\nx = 1 --[[ b ]] + 2\n",
        "x = 1 + 2\n"
    ));
    assert!(passes(
        "e/s.wgsl",
        "/* a /* b */ */\nlet x = 1.0; // c\n",
        "let x = 1.0;\n"
    ));
}

#[test]
fn a_code_change_fails_naming_its_file_and_line() {
    for (old, new) in [
        ("let s = \"a  b\";\n", "let s = \"a b\";\n"),
        ("fn add() {}\n", "fn sum() {}\n"),
        ("#[inline]\nfn f() {}\n", "fn f() {}\n"),
        ("let u = \"// x\";\n", "let u = \"// y\";\n"),
        ("let a = b;\n", "let ab;\n"),
    ] {
        let (found, _) = prove(&[("src/a.rs", old)], &[("src/a.rs", Some(new))]);
        assert_eq!(found.len(), 1, "{old:?} -> {new:?}");
        assert!(
            found[0].starts_with("src/a.rs:1: [comment-only] the code differs"),
            "{}",
            found[0]
        );
    }
    assert!(!passes("e/s.lua", "s = '-- a'\n", "s = ''\n"));
    assert!(!passes("e/s.lua", "s = [[\n  a\n]]\n", "s = [[\n a\n]]\n"));
}

#[test]
fn another_type_and_an_added_deleted_or_renamed_file_fail() {
    let (found, _) = prove(&[("a.md", "x\n")], &[("a.md", Some("y\n"))]);
    assert!(found[0].contains("not Rust, Lua or WGSL"), "{found:?}");
    let (found, _) = prove(&[("a.rs", "fn a() {}\n")], &[("b.rs", Some("// new\n"))]);
    assert!(
        found[0].starts_with("b.rs: [comment-only] added"),
        "{found:?}"
    );
    let (found, _) = prove(
        &[("a.rs", "fn a() {}\n"), ("b.rs", "fn b() {}\n")],
        &[("b.rs", None)],
    );
    assert!(
        found[0].starts_with("b.rs: [comment-only] deleted"),
        "{found:?}"
    );
    let body = "fn a() { let long = 1; let longer = 2; }\n";
    let repo_files = [("a.rs", body)];
    let (found, _) = prove(&repo_files, &[("a.rs", None), ("c.rs", Some(body))]);
    assert!(
        found[0].starts_with("c.rs: [comment-only] renamed"),
        "{found:?}"
    );
}

#[test]
fn it_counts_the_fenced_examples_the_diff_removes() {
    let old = "/// ```\n/// f();\n/// ```\nfn f() {}\n";
    let (found, body) = prove(&[("a.rs", old)], &[("a.rs", Some("fn f() {}\n"))]);
    assert!(found.is_empty(), "{found:?}");
    assert!(body.contains("1 file(s) changed since"), "{body}");
    assert!(body.contains("1 fenced doc example(s) removed"), "{body}");
}
