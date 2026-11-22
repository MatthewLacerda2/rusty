mod escapes;

use crate::code::code_lines;
use crate::gates::Verdict;
use std::fs;
use std::path::{Path, PathBuf};

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
    // Process-wide state, banned crate-wide so no test races another on it.
    "std::env::set_var",
    "std::env::remove_var",
    "std::env::set_current_dir",
];

const DENIES: &[&str] = &[
    "disallowed_methods = \"deny\"",
    "disallowed_types = \"deny\"",
    "allow_attributes = \"deny\"",
    "allow_attributes_without_reason = \"deny\"",
];

const SETTINGS: &[&str] = &["allow-unwrap-in-tests = true"];

const OPT_OUTS: &[&str] = &[
    "clippy::disallowed_methods",
    "clippy::disallowed_types",
    "clippy::style",
    "clippy::all",
];

/// The rule a `lint: allow(…)` names to excuse one statement's opt-out, meant for
/// process-wide state (`set_current_dir`): the gate does not check which call it is.
const STATEMENT_OPT_OUT: &str = "statement-opt-out";

pub fn verdict(base: &Path, _: &[String]) -> Verdict {
    let read = |p: &Path| fs::read_to_string(p).unwrap_or_default();
    let root_manifest = read(&base.join("Cargo.toml"));
    let mut violations = config_violations(&read(&base.join("clippy.toml")), &root_manifest);
    violations.extend(member_violations("Cargo.toml", &root_manifest));
    for manifest in crate::modules::member_manifests(base) {
        let at = relative(base, &manifest);
        violations.extend(member_violations(&at, &read(&manifest)));
    }
    let roots = crate::layers::platform_roots();
    let mut files = Vec::new();
    walk(&base.join("src"), &mut files);
    walk(&base.join(crate::modules::CRATES), &mut files);
    files.sort();
    for path in files {
        let norm = relative(base, &path);
        violations.extend(opt_out_violations(&norm, &read(&path), &roots));
    }
    let mut files = Vec::new();
    for root in escapes::ROOTS {
        walk(&base.join(root), &mut files);
    }
    files.sort();
    for path in files {
        violations.extend(escapes::violations(&relative(base, &path), &read(&path)));
    }
    Verdict::listed("determinism", violations, |n| {
        format!(
            "\n{n} hole(s) in the lint policy. Restore clippy.toml / Cargo.toml, move a clock \
             read to a platform module (its root carries the allow), or fix the code a \
             locked lint flags (docs/linting.md).\n"
        )
    })
}

fn relative(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn config_violations(clippy_toml: &str, cargo_toml: &str) -> Vec<String> {
    let unset = SETTINGS
        .iter()
        .filter(|s| !clippy_toml.lines().any(|l| l.trim() == **s))
        .map(|s| format!("UNSET `{s}` — missing from clippy.toml"));
    let missing = BANNED
        .iter()
        .filter(|p| !clippy_toml.contains(&format!("path = \"{p}\"")))
        .map(|p| format!("UNBANNED `{p}` — clippy.toml no longer lists it"));
    let undenied = DENIES
        .iter()
        .filter(|d| !cargo_toml.lines().any(|l| l.trim() == **d))
        .map(|d| format!("UNDENIED `{d}` — missing from Cargo.toml's [workspace.lints.clippy]"));
    missing.chain(undenied).chain(unset).collect()
}

fn member_violations(path: &str, manifest: &str) -> Vec<String> {
    let mut section = "";
    let inherits = manifest.lines().any(|l| {
        let l = l.split('#').next().unwrap_or("").trim();
        if l.starts_with('[') {
            section = l;
        }
        section == "[lints]" && l.replace(' ', "") == "workspace=true"
    });
    if inherits {
        Vec::new()
    } else {
        vec![format!(
            "UNINHERITED {path} — every package takes the workspace's lint policy: \
             add `[lints]` with `workspace = true`"
        )]
    }
}

fn opt_out_violations(path: &str, content: &str, roots: &[String]) -> Vec<String> {
    if roots
        .iter()
        .any(|r| *r == crate::modules::module_path(path))
    {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, code) in code_lines(content).iter().enumerate() {
        if crate::rules::allowed(content, i + 1, STATEMENT_OPT_OUT) {
            continue;
        }
        for name in OPT_OUTS.iter().filter(|n| names_lint(code, n)) {
            out.push(format!(
                "OPT_OUT {path}:{} `{name}` — only a platform module's root may opt out of the clock ban",
                i + 1
            ));
        }
    }
    out
}

fn names_lint(code: &str, name: &str) -> bool {
    code.match_indices(name).any(|(at, _)| {
        let next = code[at + name.len()..].chars().next();
        !next.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    if dir.is_file() {
        out.push(dir.to_path_buf());
        return;
    }
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

#[cfg(test)]
mod escapes_tests;
#[cfg(test)]
mod tests;
