use super::names::{Left, Names, Renamer};
use crate::code::{code_of, Open};

#[derive(Debug, PartialEq, Eq)]
pub struct Bin {
    pub name: String,
    pub path: String,
    pub features: Vec<String>,
}

impl Bin {
    pub fn module(&self) -> String {
        format!("__bin_{}", self.name.replace('-', "_"))
    }
}

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

/// A folded caller's file. In its `root` file only, the inner attributes at its head
/// are blanked, line numbers kept: a module cannot carry them. One in a macro body
/// (`proptest!`'s `#![proptest_config(..)]`) is the macro's input and stays.
pub fn caller(
    source: &str,
    module: &str,
    lib_name: &str,
    root: bool,
    members: &[(String, String)],
) -> String {
    fold_file(source, module, lib_name, root, members).0
}

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
    let mut renamer = Renamer::new(&names);
    for (i, line) in source.lines().enumerate() {
        let mut brackets = 0i32;
        let code = code_of(line, &mut open, |_, c| match c {
            '[' => brackets += 1,
            ']' => brackets -= 1,
            _ => {}
        });
        let attribute = depth > 0 || code.trim_start().starts_with("#![");
        head &= attribute || code.trim().is_empty();
        let (line, here) = renamer.line(line);
        if head && attribute {
            depth += brackets;
            out.push('\n');
            continue;
        }
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
