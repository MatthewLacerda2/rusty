//! The rewrite that makes rustc's `dead_code` see the whole project (#1003, #940).
//!
//! `src/lib.rs` exports every module, so no `pub` item is ever dead to rustc. On a
//! throwaway copy: the modules go `pub(crate)`, and every real caller is folded
//! into the lib as a module, so its uses still count — each `[[bin]]` as
//! `__bin_<name>` (behind its `required-features`), and `tests/main.rs` as
//! `#[cfg(test)] __itests`. Their sources then name the lib as `crate::` and their
//! own root as `crate::<module>::`.

use crate::code::code_lines;

/// One `[[bin]]` target of `Cargo.toml`.
#[derive(Debug, PartialEq, Eq)]
pub struct Bin {
    pub name: String,
    /// Package-relative, e.g. `src/bin/play.rs`.
    pub path: String,
    pub features: Vec<String>,
}

impl Bin {
    /// The module the bin becomes inside the lib.
    pub fn module(&self) -> String {
        format!("__bin_{}", self.name.replace('-', "_"))
    }
}

/// The `[[bin]]` targets of a `Cargo.toml` (the simple one-key-per-line form).
pub fn bins(cargo_toml: &str) -> Vec<Bin> {
    let mut out: Vec<Bin> = Vec::new();
    let mut in_bin = false;
    for line in cargo_toml.lines().map(str::trim) {
        if line.starts_with('[') {
            in_bin = line == "[[bin]]";
            if in_bin {
                out.push(Bin {
                    name: String::new(),
                    path: String::new(),
                    features: Vec::new(),
                });
            }
            continue;
        }
        let (Some(bin), Some((key, value))) =
            (out.last_mut().filter(|_| in_bin), line.split_once('='))
        else {
            continue;
        };
        let quoted = || value.split('"').skip(1).step_by(2).map(String::from);
        match key.trim() {
            "name" => bin.name = quoted().next().unwrap_or_default(),
            "path" => bin.path = quoted().next().unwrap_or_default(),
            "required-features" => bin.features = quoted().collect(),
            _ => {}
        }
    }
    out
}

/// `lib.rs` with its modules crate-private and every caller folded in.
pub fn lib(source: &str, bins: &[Bin], tests_root: Option<&str>) -> String {
    let mut out: String = source
        .lines()
        .map(|l| match l.strip_prefix("pub mod ") {
            Some(rest) => format!("pub(crate) mod {rest}\n"),
            None => format!("{l}\n"),
        })
        .collect();
    for bin in bins {
        if !bin.features.is_empty() {
            let all: Vec<String> = bin
                .features
                .iter()
                .map(|f| format!("feature = \"{f}\""))
                .collect();
            out.push_str(&format!("#[cfg(all({}))]\n", all.join(", ")));
        }
        let path = bin
            .path
            .strip_prefix("src/")
            .map_or(format!("../{}", bin.path), String::from);
        out.push_str(&format!(
            "#[path = \"{path}\"]\npub mod {};\n",
            bin.module()
        ));
    }
    if let Some(root) = tests_root {
        out.push_str(&format!(
            "#[cfg(test)]\n#[path = \"../{root}\"]\nmod __itests;\n"
        ));
    }
    out
}

/// One source file of a folded caller, now the lib's module `module`: its own
/// root is `crate::<module>::`, the lib (`lib_name::`) is `crate::`, `main` is
/// callable, and the inner attributes at its head (only a crate root may carry
/// some) are blanked, keeping every line number. One inside a macro body, such
/// as `proptest!`'s `#![proptest_config(..)]`, is the macro's input and stays
/// (#1205).
pub fn caller(source: &str, module: &str, lib_name: &str) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    let mut head = true;
    for (line, code) in source.lines().zip(code_lines(source)) {
        let attribute = depth > 0 || code.trim_start().starts_with("#![");
        head &= attribute || code.trim().is_empty();
        if head && attribute {
            depth += code.matches('[').count() as i32 - code.matches(']').count() as i32;
            out.push('\n');
            continue;
        }
        let line = line
            .replace("crate::", &format!("crate::{module}::"))
            .replace(&format!("{lib_name}::"), "crate::");
        let line = match line.strip_prefix("fn main(") {
            Some(rest) => format!("pub fn main({rest}"),
            None => line,
        };
        out.push_str(&line);
        out.push('\n');
    }
    out
}
