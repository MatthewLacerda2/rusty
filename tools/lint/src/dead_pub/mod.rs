//! `--dead-pub`: `pub` items nothing in the project uses (#1003). A **signal**,
//! never a gate: it prints and exits 0, and exits 1 only when it produced no report.
//!
//! `src/lib.rs` makes every module `pub mod`, so rustc's `dead_code` never fires on
//! a `pub` item. On a copy of the tree under `target/dead-pub/` (the worktree is
//! never touched) [`fold`] makes the modules crate-private and folds every bin and
//! the integration suite into the lib, then `cargo check --lib` runs twice with
//! `--features dev`: with `--profile test` (dead even counting tests) and without
//! (the difference is what only tests use). Both share this worktree's `target/`,
//! so only the copied crate itself compiles. Code behind `not(feature = "dev")` is
//! not seen. The recipe and what it found first are #940's.
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --dead-pub` (`make dead-pub`)

pub mod fold;
pub mod platform;
pub mod report;

use std::fs;
use std::io;
use std::path::Path;
use std::process::{exit, Command};

const REPORT: &str = ".lint/dead-pub.txt";
/// Never copied: build output, git, the operator's worktrees and run output.
const SKIP: &[&str] = &["target", ".git", ".claude", ".lint", "out"];

/// Entry point, from the repo root.
pub fn run() {
    let root = std::env::current_dir().expect("current dir");
    let tree = root.join("target/dead-pub/tree");
    let target = root.join("target");
    let result = prepare(&root, &tree, "rusty").and_then(|()| {
        let with_tests = check(&tree, &target, &["--profile", "test"], &["dev"])?;
        let without_tests = check(&tree, &target, &[], &["dev"])?;
        Ok(report::render(&with_tests, &without_tests, &|p| {
            platform::is_gated(&tree, p)
        }))
    });
    match result {
        Ok(body) => {
            fs::create_dir_all(".lint").ok();
            fs::write(REPORT, &body).ok();
            print!("{body}");
        }
        Err(e) => {
            eprintln!("dead-pub: no report: {e}");
            exit(1);
        }
    }
}

/// Copy `root` to `tree` (a fresh copy) and fold its callers into its lib.
pub fn prepare(root: &Path, tree: &Path, lib_name: &str) -> io::Result<()> {
    if tree.exists() {
        fs::remove_dir_all(tree)?;
    }
    copy(root, tree)?;
    let bins = fold::bins(&fs::read_to_string(tree.join("Cargo.toml"))?);
    let tests = tree
        .join("tests/main.rs")
        .exists()
        .then_some("tests/main.rs");
    let lib = tree.join("src/lib.rs");
    fs::write(&lib, fold::lib(&fs::read_to_string(&lib)?, &bins, tests))?;
    for bin in &bins {
        let file = tree.join(&bin.path);
        rewrite(&file, &bin.module(), lib_name)?;
        rewrite_dir(&file.with_extension(""), &bin.module(), lib_name)?;
    }
    if tests.is_some() {
        rewrite_dir(&tree.join("tests"), "__itests", lib_name)?;
    }
    Ok(())
}

/// `cargo check --lib` of `tree`: the unused-item warnings, or an error when it failed.
pub fn check(
    tree: &Path,
    target: &Path,
    profile: &[&str],
    features: &[&str],
) -> io::Result<std::collections::BTreeSet<report::Warning>> {
    let mut cargo = Command::new("cargo");
    cargo
        .current_dir(tree)
        .args(["check", "--lib", "--message-format", "short"]);
    cargo.arg("--target-dir").arg(target).args(profile);
    if tree.join("Cargo.lock").exists() {
        cargo.arg("--locked");
    }
    if !features.is_empty() {
        cargo.args(["--features", &features.join(",")]);
    }
    let out = cargo.output()?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    let (warnings, failed) = report::parse(&stderr);
    if failed || !out.status.success() {
        return Err(io::Error::other(format!(
            "cargo check failed on the folded copy:\n{stderr}"
        )));
    }
    Ok(warnings)
}

fn copy(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if SKIP.iter().any(|s| name == *s) {
            continue;
        }
        if entry.file_type()?.is_dir() {
            copy(&entry.path(), &to.join(&name))?;
        } else if entry.file_type()?.is_file() {
            fs::copy(entry.path(), to.join(&name))?;
        }
    }
    Ok(())
}

fn rewrite(file: &Path, module: &str, lib_name: &str) -> io::Result<()> {
    let source = fs::read_to_string(file)?;
    fs::write(file, fold::caller(&source, module, lib_name))
}

/// Every `.rs` under `dir` (when it exists) rewritten as part of `module`.
fn rewrite_dir(dir: &Path, module: &str, lib_name: &str) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            rewrite_dir(&path, module, lib_name)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            rewrite(&path, module, lib_name)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
