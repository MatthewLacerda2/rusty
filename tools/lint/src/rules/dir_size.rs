//! on a plain `//` comment line anywhere in it: `// lint: allow(dir-size) <why>`.

use super::finding::{parse_allow, Finding, Form};
use super::prefix::owner_of;
use super::scan::Scan;

pub const RULE: &str = "dir-size";
pub const MAX_FILES_PER_DIR: usize = 12;

pub fn sources<'a>(dir: &str, files: &[&'a str]) -> Vec<&'a str> {
    files
        .iter()
        .copied()
        .filter(|f| f.ends_with(".rs") && !matches!(*f, "mod.rs" | "main.rs" | "lib.rs"))
        .filter(|f| !crate::size::is_test_file(&format!("{dir}/{f}")))
        .collect()
}

fn allow_in(owner: &str) -> Option<(usize, bool)> {
    owner.lines().enumerate().find_map(|(i, l)| {
        let l = l.trim_start();
        if !l.starts_with("//") || l.starts_with("///") || l.starts_with("//!") {
            return None;
        }
        let (_, why) = parse_allow(l, Form::Code).filter(|(r, _)| *r == RULE)?;
        Some((i + 1, !why.is_empty()))
    })
}

pub fn check_dir(dir: &str, files: &[&str], owner: Option<(&str, &str)>) -> Option<Finding> {
    let count = sources(dir, files).len();
    let allow = owner.and_then(|(path, text)| Some((path, allow_in(text)?)));
    let at = |path: &str, line: usize, message: String| Finding {
        path: path.to_string(),
        line,
        rule: RULE,
        message,
    };
    match allow {
        Some((_, (_, false))) => None,
        Some((path, (line, true))) if count <= MAX_FILES_PER_DIR => Some(at(
            path,
            line,
            format!("{dir}/ holds {count} source files, within the cap: drop this allow"),
        )),
        Some(_) => None,
        None if count > MAX_FILES_PER_DIR => Some(match owner {
            Some((path, _)) => at(
                path,
                1,
                format!(
                    "{dir}/ holds {count} source files (cap {MAX_FILES_PER_DIR}): cluster them by \
                     responsibility into subfolders, or, for a flat registry of one file per \
                     public unit, say so with `// lint: allow(dir-size) <why>` in this file"
                ),
            ),
            None => at(
                &format!("{dir}/"),
                1,
                format!(
                    "{dir}/ holds {count} source files (cap {MAX_FILES_PER_DIR}) and has no \
                     owner file to carry an allow: cluster them by responsibility into \
                     subfolders"
                ),
            ),
        }),
        None => None,
    }
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.dirs()
        .iter()
        .filter(|(d, _)| {
            let at = crate::modules::module_path(d);
            at.starts_with("src/") || at == "src"
        })
        .filter_map(|(d, names)| {
            let owner = owner_of(&scan.base, d);
            let owner = owner.as_deref().map(|p| (p, scan.source(p)));
            check_dir(d, names, owner)
        })
        .collect()
}

#[cfg(test)]
#[path = "dir_size_tests.rs"]
mod tests;
