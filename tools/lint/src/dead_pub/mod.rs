pub mod fold;
pub mod names;
pub mod platform;
pub mod report;
mod scope;
pub mod workspace;

use std::fs;
use std::io;
use std::path::Path;
use std::process::{exit, Command};

const REPORT: &str = ".lint/dead-pub.txt";
/// Never copied: build output, git, the worktrees under `.claude/`, reports and run output.
const SKIP: &[&str] = &["target", ".git", ".claude", ".lint", "out"];

pub fn run() {
    let root = std::env::current_dir().expect("current dir");
    let tree = root.join("target/dead-pub/tree");
    let target = root.join("target");
    let result = prepare(&root, &tree, "rusty").and_then(|()| {
        // `dev` on in both: code behind `not(feature = "dev")` is never checked.
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

fn write_report(report: &Path, body: &str) -> io::Result<()> {
    if let Some(dir) = report.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(report, body)
}

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

pub fn check(
    tree: &Path,
    target: &Path,
    profile: &[&str],
    features: &[&str],
) -> io::Result<std::collections::BTreeSet<report::Warning>> {
    let mut cargo = Command::new("cargo");
    // Folded into the lib, an integration test's `env!("CARGO_BIN_EXE_<bin>")` is
    // unset (cargo sets it for test targets only); the binary never runs, so any path.
    for bin in fold::bins(&fs::read_to_string(tree.join("Cargo.toml"))?) {
        let exe = target.join("dead-pub/never-run").join(&bin.name);
        cargo.env(format!("CARGO_BIN_EXE_{}", bin.name), exe);
    }
    cargo
        .current_dir(tree)
        .args(["check", "--lib", "--message-format", "short"]);
    // Not `--locked`: the fold rewrote the root's dependencies, so their lock entry moves.
    // The worktree's own `target/`, so only the folded crate compiles afresh.
    cargo.arg("--target-dir").arg(target).args(profile);
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
