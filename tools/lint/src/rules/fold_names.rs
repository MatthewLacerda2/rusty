use super::finding::Finding;
use super::scan::Scan;
use crate::dead_pub::{fold, workspace};
use std::fs;

pub const RULE: &str = "fold-names";

pub fn run(scan: &Scan) -> Vec<Finding> {
    let read = |p: &str| fs::read_to_string(scan.root().join(p)).unwrap_or_default();
    let manifest = read("Cargo.toml");
    let Some(lib) = workspace::package_name(&manifest).map(|n| n.replace('-', "_")) else {
        return Vec::new();
    };
    let members = match workspace::named(scan.root(), &read("src/lib.rs")) {
        Ok(members) => members,
        Err(why) => {
            return vec![Finding {
                path: "Cargo.toml".to_string(),
                line: 1,
                rule: RULE,
                message: format!(
                    "the workspace's members cannot be read, so no file can be checked \
                     for a name the dead-pub fold cannot rewrite: {why}; fix the manifest \
                     (nothing excuses this finding)"
                ),
            }]
        }
    };
    let names: Vec<(String, String)> = members
        .iter()
        .map(|m| (m.ident.clone(), m.module.clone()))
        .collect();
    // What the fold rewrites. Not the root's own lib: the fold mounts its re-exports.
    let mut folded: Vec<String> = members.iter().map(|m| format!("{}/src/", m.dir)).collect();
    folded.push("tests/".to_string());
    for bin in fold::bins(&manifest) {
        folded.push(bin.path.trim_end_matches(".rs").to_string() + "/");
        folded.push(bin.path);
    }
    scan.each_file(|path, source| {
        if !folded
            .iter()
            .any(|f| path == f || (f.ends_with('/') && path.starts_with(f.as_str())))
        {
            return Vec::new();
        }
        check(path, source, &lib, &names)
    })
}

pub fn check(path: &str, source: &str, lib: &str, members: &[(String, String)]) -> Vec<Finding> {
    fold::unfoldable(source, "m", lib, members)
        .into_iter()
        .map(|(line, left)| Finding {
            path: path.to_string(),
            line,
            rule: RULE,
            message: format!(
                "the dead-pub fold cannot rewrite `{}` here, so `make dead-pub` stops \
                 compiling: {}. Every other spelling folds: a path, or a bare name in \
                 `use … as …;`, a `use` group, `extern crate`, an attribute or a macro \
                 argument (tools/lint/src/dead_pub/names.rs)",
                left.name, left.why
            ),
        })
        .collect()
}

#[cfg(test)]
#[path = "fold_names_tests.rs"]
mod tests;
