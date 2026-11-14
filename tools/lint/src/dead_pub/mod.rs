//! `--dead-pub`: `pub` items nothing in the project uses. A **signal**,
//! never a gate: it prints and exits 0, and exits 1 only when it produced no report.
//!
//! A `pub` item across a crate boundary is never dead to rustc, and the engine is a
//! workspace of one crate per module. So on a copy of the tree under
//! `target/dead-pub/` (the worktree is never touched) the fold rebuilds the one-crate
//! shape for the check only: [`workspace`] mounts every member crate as a
//! crate-private module of the root's lib and gives the root their dependencies,
//! [`fold`] folds every bin and the integration suite in as modules, and every
//! `rusty_<m>::` path becomes `crate::<m>::`. A use from another crate is then a use
//! rustc sees. `cargo check --lib` runs twice with `--features dev`: with `--profile
//! test` (dead even counting tests) and without (the difference is what only tests
//! use). Both share this worktree's `target/`, so only the copied crate compiles.
//! Code behind `not(feature = "dev")` is not seen. A fold that made nothing
//! crate-private is an error, not an empty report: a layout the fold does not know
//! would otherwise read as "nothing dead". It runs weekly on `main`
//! (`cargo-test.yml`'s `dead-pub` job), so a fold that stops compiling goes red
//! there instead of silently.
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --dead-pub` (`make dead-pub`)

pub mod fold;
pub mod platform;
pub mod report;
pub mod workspace;

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
            print!("{body}");
            if let Err(e) = write_report(Path::new(REPORT), &body) {
                eprintln!("dead-pub: no report: could not write {REPORT}: {e}");
                exit(1);
            }
        }
        Err(e) => {
            eprintln!("dead-pub: no report: {e}");
            exit(1);
        }
    }
}

/// `body` into `report`, its directory made first.
fn write_report(report: &Path, body: &str) -> io::Result<()> {
    if let Some(dir) = report.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(report, body)
}

/// Copy `root` to `tree` (a fresh copy) and fold its members and callers into its
/// lib. An error when the fold made nothing crate-private: then rustc would see
/// every `pub` item as used and the report would be empty for the wrong reason.
pub fn prepare(root: &Path, tree: &Path, lib_name: &str) -> io::Result<()> {
    if tree.exists() {
        fs::remove_dir_all(tree)?;
    }
    copy(root, tree)?;
    let manifest = tree.join("Cargo.toml");
    let lib = tree.join("src/lib.rs");
    let members = workspace::members(tree, &fs::read_to_string(&lib)?)?;
    let names: Vec<(String, String)> = members
        .iter()
        .map(|m| (m.ident.clone(), m.module.clone()))
        .collect();
    let bins = fold::bins(&fs::read_to_string(&manifest)?);
    let tests = tree
        .join("tests/main.rs")
        .exists()
        .then_some("tests/main.rs");
    let (body, private) = fold::lib(&fs::read_to_string(&lib)?, &bins, tests);
    if private + members.len() == 0 {
        return Err(io::Error::other(
            "the fold made nothing crate-private: the lib has no `pub mod` and the \
             workspace no member, so every `pub` item would read as used. The layout \
             changed under tools/lint/src/dead_pub/: teach the fold the new one",
        ));
    }
    fs::write(&lib, workspace::mount(&body, &members))?;
    if !members.is_empty() {
        let mut all = vec![workspace::deps(&fs::read_to_string(&manifest)?)];
        let mut packages = Vec::new();
        for m in &members {
            let toml = fs::read_to_string(tree.join(&m.dir).join("Cargo.toml"))?;
            packages.extend(workspace::package_name(&toml));
            all.push(workspace::deps(&toml));
        }
        let root_toml = fs::read_to_string(&manifest)?;
        fs::write(&manifest, workspace::manifest(&root_toml, &all, &packages))?;
    }
    let caller = |file: &Path, module: &str, root: bool| -> io::Result<()> {
        let source = fs::read_to_string(file)?;
        fs::write(file, fold::caller(&source, module, lib_name, root, &names))
    };
    for m in &members {
        let src = tree.join(&m.dir).join("src");
        each_rs(&src, &mut |f| caller(f, &m.module, false))?;
        let mut lib = fs::read_to_string(src.join("lib.rs"))?;
        lib.push_str(&workspace::macro_reexport(m));
        fs::write(src.join("lib.rs"), lib)?;
    }
    for bin in &bins {
        let file = tree.join(&bin.path);
        caller(&file, &bin.module(), true)?;
        each_rs(&file.with_extension(""), &mut |f| {
            caller(f, &bin.module(), false)
        })?;
    }
    if tests.is_some() {
        let tests = tree.join("tests");
        let main = tests.join("main.rs");
        each_rs(&tests, &mut |f| caller(f, "__itests", f == main))?;
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
    // Folded into the lib, an integration test's `env!("CARGO_BIN_EXE_<bin>")`
    // is not defined: cargo sets it only for an integration test target. rustc
    // reads `env!` from its environment, and the folded binary never runs, so
    // any value will do (#1205).
    for bin in fold::bins(&fs::read_to_string(tree.join("Cargo.toml"))?) {
        let exe = target.join("dead-pub/never-run").join(&bin.name);
        cargo.env(format!("CARGO_BIN_EXE_{}", bin.name), exe);
    }
    cargo
        .current_dir(tree)
        .args(["check", "--lib", "--message-format", "short"]);
    cargo.arg("--target-dir").arg(target).args(profile);
    // Not `--locked`: the fold rewrote the root's dependency list, so its entry in
    // the copied `Cargo.lock` changes. Every other entry, version and git rev stays
    // what the lock says: cargo re-resolves only what the manifest moved.
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

/// `visit` on every `.rs` under `dir`, when it exists.
fn each_rs(dir: &Path, visit: &mut dyn FnMut(&Path) -> io::Result<()>) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            each_rs(&path, visit)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            visit(&path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
