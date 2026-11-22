use super::finding::{allowed, Finding};
use super::scan::Scan;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

#[path = "module_tree.rs"]
mod tree;
use tree::{normal, Reach};

pub const RULE: &str = "orphan-module";

pub fn run(scan: &Scan) -> Vec<Finding> {
    check(scan.root(), scan)
}

pub fn check(base: &Path, scan: &Scan) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut roots = BTreeSet::new();
    for ws in workspaces(base, &scan.files) {
        match crate_roots(base, &ws) {
            Ok(found) => roots.extend(found),
            Err(why) => findings.push(Finding {
                path: format!("{ws}Cargo.toml"),
                line: 1,
                rule: RULE,
                message: format!(
                    "`cargo metadata` cannot read this workspace, so no file under it can \
                     be checked for a missing `mod`: {why}; fix the manifest (nothing \
                     excuses this finding)"
                ),
            }),
        }
    }
    let homes: BTreeSet<String> = roots.iter().map(|r| parent(r)).collect();
    let reached = reach(base, scan, roots);
    let mut orphans: Vec<&String> = scan
        .files
        .iter()
        .filter(|f| !reached.contains(*f))
        .collect();
    orphans.sort();
    findings.extend(
        orphans
            .into_iter()
            .filter(|f| !allowed(scan.source(f), 1, RULE))
            .map(|f| orphan(f, &homes)),
    );
    findings
}

fn workspaces(base: &Path, files: &[String]) -> Vec<String> {
    let mut dirs = BTreeSet::new();
    for f in files {
        let mut at = Path::new(f.as_str());
        while let Some(parent) = at.parent().filter(|p| !p.as_os_str().is_empty()) {
            dirs.insert(format!("{}/", normal(parent)));
            at = parent;
        }
    }
    let manifest = |dir: &str| fs::read_to_string(base.join(dir).join("Cargo.toml")).ok();
    let mut out: Vec<String> = manifest("").map(|_| String::new()).into_iter().collect();
    out.extend(
        dirs.into_iter()
            .filter(|d| manifest(d).is_some_and(|m| m.lines().any(opens_workspace))),
    );
    out
}

fn opens_workspace(line: &str) -> bool {
    line.split('#')
        .next()
        .is_some_and(|l| l.trim() == "[workspace]")
}

fn crate_roots(base: &Path, dir: &str) -> Result<Vec<String>, String> {
    let manifest = base.join(dir).join("Cargo.toml");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    // lint: allow(subprocess) the crate roots are cargo's to say, `path =` targets included
    let out = Command::new(cargo)
        .args([
            "metadata",
            "--no-deps",
            "--offline",
            "--format-version",
            "1",
        ])
        .arg("--manifest-path")
        .arg(&manifest)
        .output()
        .map_err(|e| format!("cargo did not start: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.trim().lines().next().unwrap_or("it failed").to_string());
    }
    let at = fs::canonicalize(base).map_err(|e| e.to_string())?;
    let json = String::from_utf8_lossy(&out.stdout);
    Ok(src_paths(&json)
        .iter()
        .filter_map(|p| {
            let p = fs::canonicalize(p).unwrap_or_else(|_| p.into());
            Some(normal(p.strip_prefix(&at).ok()?))
        })
        .collect())
}

pub fn src_paths(json: &str) -> Vec<String> {
    const KEY: &str = "\"src_path\":\"";
    json.match_indices(KEY)
        .filter_map(|(i, _)| json_string(&json[i + KEY.len()..]))
        .collect()
}

fn json_string(s: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                'n' => out.push('\n'),
                't' => out.push('\t'),
                c => out.push(c),
            },
            c => out.push(c),
        }
    }
    None
}

fn reach(base: &Path, scan: &Scan, roots: BTreeSet<String>) -> BTreeSet<String> {
    let (mut parsed, mut reached) = (BTreeSet::new(), BTreeSet::new());
    let mut queue: Vec<(String, bool)> = roots.into_iter().map(|r| (r, true)).collect();
    while let Some((path, mod_rs)) = queue.pop() {
        // Once per way it is loaded, owning its folder or not: each resolves differently.
        if !parsed.insert((path.clone(), mod_rs)) {
            continue;
        }
        reached.insert(path.clone());
        let text = match scan.source(&path) {
            "" => fs::read_to_string(base.join(&path)).unwrap_or_default(),
            text => text.to_string(),
        };
        for r in tree::reached(&path, mod_rs, &text, |p| base.join(p).is_file()) {
            match r {
                Reach::Module { path, mod_rs } => queue.push((path, mod_rs)),
                Reach::Included(path) => {
                    // Read, never parsed: if it is also a module, its own `mod` reaches it.
                    reached.insert(path);
                }
            }
        }
    }
    reached
}

fn parent(path: &str) -> String {
    Path::new(path).parent().map_or(String::new(), normal)
}

fn orphan(path: &str, homes: &BTreeSet<String>) -> Finding {
    let p = Path::new(path);
    let (name, owned) = match p.file_name().and_then(|n| n.to_str()) {
        Some("mod.rs") => (p.parent(), p.parent().and_then(Path::parent)),
        _ => (Some(p), p.parent()),
    };
    let name = name
        .and_then(Path::file_stem)
        .map_or(String::new(), |n| n.to_string_lossy().into_owned());
    let owner = owned.map_or(String::new(), normal);
    let under =
        |home: &String| home.is_empty() || format!("{owner}/").starts_with(&format!("{home}/"));
    let fix = if homes.iter().any(under) {
        format!(
            "declare `mod {name};` in the module that owns `{owner}/` (its `mod.rs`, the \
             `{owner}.rs` beside it, or the crate root there)"
        )
    } else {
        "no crate root sits above it, so no plain `mod` can reach it: make it a target \
         (`path =` in a `Cargo.toml`) or name it with `#[path]` from a crate"
            .to_string()
    };
    Finding {
        path: path.to_string(),
        line: 1,
        rule: RULE,
        message: format!(
            "no `mod` declaration reaches this file, so cargo never compiles it and its \
             tests never run: {fix}; delete it if it is dead, or, if it is built another \
             way, say why on its first line with `// lint: allow(orphan-module) <why>`"
        ),
    }
}

#[cfg(test)]
#[path = "orphan_module_tests.rs"]
mod tests;
