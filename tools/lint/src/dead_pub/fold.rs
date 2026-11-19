//! The rewrite that makes rustc's `dead_code` see the whole project (#1003, #940).
//!
//! An exported module's `pub` item is never dead to rustc. On a throwaway copy: a
//! lib's `pub mod`s go `pub(crate)` (the workspace's members are mounted the same
//! way by `super::workspace`), and every real caller is folded
//! into the lib as a module, so its uses still count — each `[[bin]]` as
//! `__bin_<name>` (behind its `required-features`), and `tests/main.rs` as
//! `#[cfg(test)] __itests`. Their sources then name the lib as `crate::` and their
//! own root as `crate::<module>::`. A workspace member is folded the same way
//! (`super::workspace`): its sources are a caller whose module is the member's, and
//! every member's `rusty_<m>::` is `crate::<m>::` in every folded file, and so is
//! a member named bare (`use rusty_<m> as x;`, [`super::names`]).

use super::names::{rename, Left, Names};
use crate::code::{code_of, Open};

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

/// `lib.rs` with its modules crate-private and every caller folded in, and how
/// many `pub mod`s it made crate-private.
pub fn lib(source: &str, bins: &[Bin], tests_root: Option<&str>) -> (String, usize) {
    let mut private = 0;
    let mut out: String = source
        .lines()
        .map(|l| match l.strip_prefix("pub mod ") {
            Some(rest) => {
                private += 1;
                format!("pub(crate) mod {rest}\n")
            }
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
    (out, private)
}

/// One source file of a folded caller, now the lib's module `module`: its own
/// root is `crate::<module>::`, the lib (`lib_name::`) is `crate::`, `main` is
/// callable, and, in the caller's `root` file only (a bin, `tests/main.rs`), the
/// inner attributes at its head are blanked, keeping every line number: only a
/// module's root can't carry them, and a submodule's own (`#![cfg(..)]`) belong to
/// it. One inside a macro body, such as `proptest!`'s `#![proptest_config(..)]`,
/// is the macro's input and stays (#1205). Brackets are counted outside literals.
pub fn caller(
    source: &str,
    module: &str,
    lib_name: &str,
    root: bool,
    members: &[(String, String)],
) -> String {
    fold_file(source, module, lib_name, root, members).0
}

/// Every name of a sibling crate or the lib in `source`, folded as
/// [`caller`] folds it, that the fold must leave: (line, what was left). A
/// folded copy holding one does not compile.
pub fn unfoldable(
    source: &str,
    module: &str,
    lib_name: &str,
    members: &[(String, String)],
) -> Vec<(usize, Left)> {
    fold_file(source, module, lib_name, false, members).1
}

fn fold_file(
    source: &str,
    module: &str,
    lib_name: &str,
    root: bool,
    members: &[(String, String)],
) -> (String, Vec<(usize, Left)>) {
    let mut out = String::new();
    let mut left = Vec::new();
    let mut depth = 0i32;
    let mut head = root;
    let mut open = Open::None;
    let own = format!("crate::{module}");
    let theirs: Vec<(&str, String)> = members
        .iter()
        .map(|(ident, m)| (ident.as_str(), format!("crate::{m}")))
        .collect();
    let mut bare = vec![(lib_name, "crate")];
    bare.extend(theirs.iter().map(|(i, m)| (*i, m.as_str())));
    let mut paths = vec![("crate", own.as_str())];
    paths.extend(bare.iter().copied());
    let names = Names { paths, bare };
    for (i, line) in source.lines().enumerate() {
        let mut brackets = 0i32;
        let mut at_start = open;
        let code = code_of(line, &mut open, |_, c| match c {
            '[' => brackets += 1,
            ']' => brackets -= 1,
            _ => {}
        });
        let attribute = depth > 0 || code.trim_start().starts_with("#![");
        head &= attribute || code.trim().is_empty();
        if head && attribute {
            depth += brackets;
            out.push('\n');
            continue;
        }
        let (line, here) = rename(line, &mut at_start, &names);
        left.extend(here.into_iter().map(|l| (i + 1, l)));
        let line = match line.strip_prefix("fn main(") {
            Some(rest) => format!("pub fn main({rest}"),
            None => line,
        };
        out.push_str(&line);
        out.push('\n');
    }
    (out, left)
}
