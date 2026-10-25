//! `dir-size`: a module directory under `src/` holds at most [`MAX_FILES_PER_DIR`]
//! source files directly in it (CLAUDE.md, *Group by subfolder*, #1367). Past that,
//! cluster the files by responsibility into subfolders.
//!
//! A source file is a `.rs` file the size gate does not count as a test
//! (`size::is_test_file`), the directory's own `mod.rs` / `main.rs` / `lib.rs`
//! aside. CLAUDE.md's aim stays ≲10 ([`super::prefix::AIM_FILES_PER_DIR`], the
//! printed signal); the gate leaves two files of room above it.
//!
//! The one exception is a flat registry, where one file is one public unit and the
//! flat list is the documentation (`src/api/`: one file per Lua namespace). It says
//! so in the directory's owner file (its `mod.rs`, or the `<dir>.rs` beside it),
//! on a plain `//` comment line anywhere in it: `// lint: allow(dir-size) <why>`.
//! A directory with no owner file (`src/bin/`) has nowhere to say it, so it takes
//! no exception and is reported as itself. The rule reads that allow
//! itself, so one past its use (the directory back under the cap) is reported.

use super::finding::{parse_allow, Finding};
use super::prefix::owner_of;
use super::scan::Scan;

pub const RULE: &str = "dir-size";
/// The hard cap: CLAUDE.md's "≲10", with room for two.
pub const MAX_FILES_PER_DIR: usize = 12;

/// The source files among one directory's file names (`dir-size`'s count).
pub fn sources<'a>(dir: &str, files: &[&'a str]) -> Vec<&'a str> {
    files
        .iter()
        .copied()
        .filter(|f| f.ends_with(".rs") && !matches!(*f, "mod.rs" | "main.rs" | "lib.rs"))
        .filter(|f| !crate::size::is_test_file(&format!("{dir}/{f}")))
        .collect()
}

/// The 1-based line of `owner`'s allow, and whether it gives a reason. Only a plain
/// `//` comment line counts: a doc comment (`///`, `//!`) quoting the syntax, or a
/// comment trailing code, is not an allow.
fn allow_in(owner: &str) -> Option<(usize, bool)> {
    owner.lines().enumerate().find_map(|(i, l)| {
        let l = l.trim_start();
        if !l.starts_with("//") || l.starts_with("///") || l.starts_with("//!") {
            return None;
        }
        let (_, why) = parse_allow(l).filter(|(r, _)| *r == RULE)?;
        Some((i + 1, !why.is_empty()))
    })
}

/// The finding for one directory, given its file names and its owner file's
/// `(path, text)`.
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
        // An allow with no reason is the shared `[allow]` finding already.
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
            // No `mod.rs`, `main.rs`, `lib.rs` or `<dir>.rs` (`src/bin/`): nothing can
            // carry an allow, so the directory itself is reported.
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

/// The rule over every scanned directory under `src/`.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.dirs()
        .iter()
        .filter(|(d, _)| d.starts_with("src/") || **d == "src")
        .filter_map(|(d, names)| {
            let owner = owner_of(d);
            let owner = owner.as_deref().map(|p| (p, scan.source(p)));
            check_dir(d, names, owner)
        })
        .collect()
}

#[cfg(test)]
#[path = "dir_size_tests.rs"]
mod tests;
