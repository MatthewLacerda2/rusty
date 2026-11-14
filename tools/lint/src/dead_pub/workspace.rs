//! The workspace half of the fold: every member crate becomes one module of the
//! root's lib, on the copy, so a `pub` item another crate names is a use rustc sees
//! and one nobody names is `dead_code` again.
//!
//! Each member's `src/lib.rs` is mounted as `pub(crate) mod <name>` (its root's
//! `pub use rusty_<m> as <name>;` re-export, or the crate's own ident when the root
//! re-exports nothing of it, behind `cfg(test)` when only `[dev-dependencies]` name
//! it). Its sources then say `crate::<name>::` for their own `crate::`, and every
//! source says `crate::<name>::` for a member's `rusty_<m>::`. The root package's
//! manifest takes every member's external dependencies, with `dev` and `editor`
//! plain features, since each member's are the same two names forwarded.

use std::fs;
use std::io;
use std::path::Path;

/// One workspace member, as the fold mounts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// Root-relative, e.g. `crates/core`.
    pub dir: String,
    /// How its sources and its dependents name it, e.g. `rusty_core`.
    pub ident: String,
    /// The module it becomes in the lib, e.g. `core`.
    pub module: String,
    /// Named only by `[dev-dependencies]`: mounted under `cfg(test)`.
    pub test_only: bool,
    /// Its `#[macro_export]` macros, which land at the crate root: its module
    /// re-exports them, so `crate::<module>::<macro>` still resolves.
    pub macros: Vec<String>,
}

/// One manifest's dependency tables, by kind.
#[derive(Debug, Default)]
pub struct Deps {
    /// `[dependencies]`: (name, line).
    pub normal: Vec<(String, String)>,
    /// `[dev-dependencies]`.
    pub dev: Vec<(String, String)>,
    /// `[target.'cfg(..)'.dependencies]`: (table header, name, line).
    pub target: Vec<(String, String, String)>,
}

/// The dependency lines of a `Cargo.toml` (one key per line, as the workspace's are).
pub fn deps(cargo_toml: &str) -> Deps {
    let mut out = Deps::default();
    let mut table = String::new();
    for line in cargo_toml.lines().map(str::trim) {
        if line.starts_with('[') {
            table = line.to_string();
            continue;
        }
        let Some((key, _)) = line.split_once('=').filter(|_| !line.starts_with('#')) else {
            continue;
        };
        let entry = (key.trim().to_string(), line.to_string());
        match table.as_str() {
            "[dependencies]" => out.normal.push(entry),
            "[dev-dependencies]" => out.dev.push(entry),
            t if t.starts_with("[target.") && t.ends_with(".dependencies]") => {
                out.target.push((table.clone(), entry.0, entry.1));
            }
            _ => {}
        }
    }
    out
}

/// The `[package] name` of a `Cargo.toml`.
pub fn package_name(cargo_toml: &str) -> Option<String> {
    let mut in_package = false;
    for line in cargo_toml.lines().map(str::trim) {
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package && line.starts_with("name") {
            return line.split('"').nth(1).map(String::from);
        }
    }
    None
}

/// The member directories `[workspace] members` names (a literal path, or `dir/*`).
pub fn member_dirs(root: &Path, cargo_toml: &str) -> io::Result<Vec<String>> {
    let Some(line) = cargo_toml
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("members"))
    else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for pattern in line.split('"').skip(1).step_by(2) {
        match pattern.strip_suffix("/*") {
            Some(dir) => {
                let mut found: Vec<String> = fs::read_dir(root.join(dir))?
                    .filter_map(Result::ok)
                    .filter(|e| e.path().join("Cargo.toml").is_file())
                    .map(|e| format!("{dir}/{}", e.file_name().to_string_lossy()))
                    .collect();
                found.sort();
                out.extend(found);
            }
            None => out.push(pattern.to_string()),
        }
    }
    Ok(out)
}

/// The members of the workspace at `root` whose `rusty_<m>` paths the fold rewrites.
pub fn members(root: &Path, root_lib: &str) -> io::Result<Vec<Member>> {
    let manifest = fs::read_to_string(root.join("Cargo.toml"))?;
    let mut found = Vec::new();
    let mut named = Vec::new();
    for dir in member_dirs(root, &manifest)? {
        let toml = fs::read_to_string(root.join(&dir).join("Cargo.toml"))?;
        let Some(name) = package_name(&toml) else {
            continue;
        };
        let d = deps(&toml);
        named.extend(
            d.normal
                .into_iter()
                .chain(d.target.into_iter().map(|t| (t.1, t.2))),
        );
        found.push((dir, name));
    }
    named.extend(deps(&manifest).normal);
    let mut out = Vec::new();
    for (dir, name) in found {
        let ident = name.replace('-', "_");
        let module = reexport_alias(root_lib, &ident).unwrap_or_else(|| ident.clone());
        let test_only = !named.iter().any(|(n, _)| *n == name);
        let macros = exported_macros(&root.join(&dir).join("src"))?;
        out.push(Member {
            dir,
            ident,
            module,
            test_only,
            macros,
        });
    }
    Ok(out)
}

/// `alias` of a `pub use <ident> as <alias>;` line in `lib`.
fn reexport_alias(lib: &str, ident: &str) -> Option<String> {
    lib.lines().find_map(|l| {
        let rest = l.trim().strip_prefix("pub use ")?.strip_prefix(ident)?;
        Some(
            rest.strip_prefix(" as ")?
                .trim_end_matches(';')
                .trim()
                .to_string(),
        )
    })
}

/// The names of the `#[macro_export]` macros under `dir`.
fn exported_macros(dir: &Path) -> io::Result<Vec<String>> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            out.extend(exported_macros(&path)?);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let source = fs::read_to_string(&path)?;
            let mut exported = false;
            for line in source.lines().map(str::trim) {
                if line == "#[macro_export]" {
                    exported = true;
                } else if let Some(rest) = line.strip_prefix("macro_rules! ").filter(|_| exported) {
                    out.push(rest.trim_end_matches(['{', '(', ' ']).to_string());
                    exported = false;
                } else if !line.starts_with("#[") && !line.starts_with("///") {
                    exported = false;
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

/// The root lib with each member mounted as a crate-private module: a re-export
/// line becomes its mount (its attributes kept), the rest are appended.
pub fn mount(lib: &str, members: &[Member]) -> String {
    let at = |m: &Member| {
        format!(
            "#[path = \"../{}/src/lib.rs\"]\npub(crate) mod {};\n",
            m.dir, m.module
        )
    };
    let mut out = String::new();
    let mut mounted = Vec::new();
    for line in lib.lines() {
        let reexport = members
            .iter()
            .find(|m| line.trim() == format!("pub use {} as {};", m.ident, m.module));
        match reexport {
            Some(m) => {
                out.push_str(&at(m));
                mounted.push(m.ident.clone());
            }
            None => out.push_str(&format!("{line}\n")),
        }
    }
    for m in members.iter().filter(|m| !mounted.contains(&m.ident)) {
        if m.test_only {
            out.push_str("#[cfg(test)]\n");
        }
        out.push_str(&at(m));
    }
    out
}

/// What a member's `lib.rs` gains at its end: its exported macros, re-exported so
/// `crate::<module>::<macro>` (once `rusty_<m>::<macro>`) still names them.
pub fn macro_reexport(member: &Member) -> String {
    if member.macros.is_empty() {
        return String::new();
    }
    format!(
        "#[allow(unused_imports, reason = \"dead-pub fold: macros land at the root\")]\npub use crate::{{{}}};\n",
        member.macros.join(", ")
    )
}

/// The root manifest with every member's external dependencies, and `dev` and
/// `editor` as plain features (`editor` on by default, as at the root).
pub fn manifest(root_toml: &str, all: &[Deps], members: &[String]) -> String {
    let external = |n: &String| !members.contains(n);
    let line = |n: &str| format!("{n} = {{ workspace = true }}\n");
    let mut normal: Vec<&String> = all
        .iter()
        .flat_map(|d| d.normal.iter().map(|e| &e.0))
        .filter(|n| external(n))
        .collect();
    normal.sort();
    normal.dedup();
    let mut dev: Vec<&String> = all
        .iter()
        .flat_map(|d| d.dev.iter().map(|e| &e.0))
        .filter(|n| external(n) && !normal.contains(n))
        .collect();
    dev.sort();
    dev.dedup();
    let mut target: Vec<(&String, &String)> = all
        .iter()
        .flat_map(|d| d.target.iter().map(|t| (&t.0, &t.1)))
        .collect();
    target.sort();
    target.dedup();
    let mut out = String::new();
    let mut skip = false;
    for l in root_toml.lines() {
        if l.starts_with('[') {
            skip = l == "[dependencies]"
                || l == "[dev-dependencies]"
                || l == "[features]"
                || (l.starts_with("[target.") && l.ends_with(".dependencies]"));
        }
        // `default-members` would make `cargo check --lib` check every member
        // again on its own; without it, only the root, which now holds them all.
        if !skip && !l.starts_with("default-members") {
            out.push_str(&format!("{l}\n"));
        }
    }
    out.push_str("\n[features]\ndefault = [\"editor\"]\ndev = []\neditor = []\n\n[dependencies]\n");
    normal.iter().for_each(|n| out.push_str(&line(n)));
    out.push_str("\n[dev-dependencies]\n");
    dev.iter().for_each(|n| out.push_str(&line(n)));
    let mut table = "";
    for (header, name) in target {
        if header != table {
            out.push_str(&format!("\n{header}\n"));
            table = header;
        }
        out.push_str(&line(name));
    }
    out
}
