//! Determinism gate — protects the headless harness's bit-determinism.
//!
//! The simulation must be a pure function of (seed, inputs, fixed dt). Wall-clock
//! reads and unseeded RNG break replayability, so they are banned from the sim —
//! and so are `std`'s hash maps, whose OS-seeded hasher is an unseeded RNG that
//! decides their iteration order (#764).
//! **Clippy enforces the ban** (#757): `clippy.toml` lists the banned items under
//! `disallowed-methods` / `disallowed-types` and `Cargo.toml` denies both lints.
//! Clippy matches resolved paths, so an alias (`use std::time::Instant as Clock`)
//! or a re-export can't slip past the way it did the old substring scan.
//!
//! The ban is crate-wide, so the platform modules that need real time opt out with
//! `#![allow(clippy::disallowed_methods, clippy::disallowed_types)]` at their module
//! root. This gate keeps that escape hatch honest:
//! - only a platform row's root (`src/<module>/mod.rs`, from the layer table,
//!   `layers/table.rs`) may name those lints, or allow the groups that hold them;
//! - `clippy.toml` still lists every banned item, and `Cargo.toml` still denies them.
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --determinism`
//! Exit code 1 on any violation; report mirrored to `.lint/report.txt`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

/// Every item the ban must cover, each as it appears in a `path = "…"` of `clippy.toml`.
const BANNED: &[&str] = &[
    "std::time::Instant::now",
    "std::time::Instant::elapsed",
    "std::time::SystemTime::now",
    "std::time::SystemTime::elapsed",
    "getrandom::fill",
    "getrandom::u32",
    "getrandom::u64",
    "rand::random",
    "rand::thread_rng",
    "rand::rng",
    "std::time::Instant",
    "std::time::SystemTime",
    "std::collections::HashMap",
    "std::collections::HashSet",
    "std::hash::RandomState",
];

/// The `Cargo.toml` `[lints.clippy]` lines that turn the ban into an error.
const DENIES: &[&str] = &[
    "disallowed_methods = \"deny\"",
    "disallowed_types = \"deny\"",
];

/// Names that switch the ban off: the two lints, and the groups that contain them.
const OPT_OUTS: &[&str] = &[
    "clippy::disallowed_methods",
    "clippy::disallowed_types",
    "clippy::style",
    "clippy::all",
];

const REPORT: &str = ".lint/report.txt";

/// Entry point: check the clippy config and every opt-out under `src/`.
pub fn run() {
    let read = |p: &str| fs::read_to_string(p).unwrap_or_default();
    let mut violations = config_violations(&read("clippy.toml"), &read("Cargo.toml"));
    let roots = crate::layers::platform_roots();
    let mut files = Vec::new();
    walk(Path::new("src"), &mut files);
    files.sort();
    for path in files {
        let norm = path.to_string_lossy().replace('\\', "/");
        let content = fs::read_to_string(&path).unwrap_or_default();
        violations.extend(opt_out_violations(&norm, &content, &roots));
    }
    report(&violations);
    if violations.is_empty() {
        println!("determinism: ok");
    } else {
        exit(1);
    }
}

/// The banned items missing from `clippy.toml`, and the lints `Cargo.toml` no longer denies.
fn config_violations(clippy_toml: &str, cargo_toml: &str) -> Vec<String> {
    let missing = BANNED
        .iter()
        .filter(|p| !clippy_toml.contains(&format!("path = \"{p}\"")))
        .map(|p| format!("UNBANNED `{p}` — clippy.toml no longer lists it"));
    let undenied = DENIES
        .iter()
        .filter(|d| !cargo_toml.lines().any(|l| l.trim() == **d))
        .map(|d| format!("UNDENIED `{d}` — missing from Cargo.toml's [lints.clippy]"));
    missing.chain(undenied).collect()
}

/// Every line of `path` that opts out of the ban, unless `path` is a platform root.
fn opt_out_violations(path: &str, content: &str, roots: &[String]) -> Vec<String> {
    if roots.iter().any(|r| r == path) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, raw) in content.lines().enumerate() {
        let code = strip_comment(raw);
        for name in OPT_OUTS.iter().filter(|n| names_lint(code, n)) {
            out.push(format!(
                "OPT_OUT {path}:{} `{name}` — only a platform module's root may opt out of the clock ban",
                i + 1
            ));
        }
    }
    out
}

/// Whether `code` names the lint `name` as a whole path (so `clippy::all` does not
/// match `clippy::all_something`).
fn names_lint(code: &str, name: &str) -> bool {
    code.match_indices(name).any(|(at, _)| {
        let next = code[at + name.len()..].chars().next();
        !next.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Drop a trailing `//` line comment so prose about the lints doesn't trip the
/// gate. Not string-literal aware — adequate for a coarse source scan.
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn report(violations: &[String]) {
    let mut body = if violations.is_empty() {
        String::from("determinism: ok\n")
    } else {
        String::from("determinism: FAILED\n")
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
            "\n{} hole(s) in the sim's clock and RNG ban. Restore clippy.toml / Cargo.toml, \
             or move the clock read to a platform module (its root carries the allow).",
            violations.len()
        );
    }
}

#[cfg(test)]
#[path = "determinism_tests.rs"]
mod tests;
