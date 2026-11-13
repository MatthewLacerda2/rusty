//! `orphan-seeds`: a saved proptest seed is one a test still reads.
//!
//! proptest keeps a failing case's seed at `<root>/proptest-regressions/<path>.txt`,
//! where `<root>` holds the folder with the test's nearest `lib.rs` or `main.rs`
//! and `<path>` is the source file's path inside that folder. A module that moves
//! into its crate (`crates/CLAUDE.md`), or a test file renamed or deleted, leaves
//! the file where no test looks for it: the seed silently stops replaying, and the
//! bug it holds can come back green (ecs's parent-cycle seed was left at the root
//! when ecs moved, #2140). So every seed file under the root's or a member crate's
//! `proptest-regressions/` must have its `<path>.rs` in a sibling folder holding a
//! `lib.rs` or `main.rs`. No allow: the fix is to move the file beside its source's
//! crate, or delete it with the test it served.

use super::finding::Finding;
use super::scan::Scan;
use std::fs;
use std::path::{Path, PathBuf};

pub const RULE: &str = "orphan-seeds";

const SEEDS: &str = "proptest-regressions";

/// The rule over the repository at `base`: the root and each `crates/<name>/`.
pub fn check(base: &Path) -> Vec<Finding> {
    let mut roots = vec![PathBuf::new()];
    roots.extend(
        children(&base.join(crate::modules::CRATES))
            .into_iter()
            .map(|c| Path::new(crate::modules::CRATES).join(c.file_name().unwrap_or_default())),
    );
    roots
        .iter()
        .flat_map(|root| check_root(base, root))
        .collect()
}

fn check_root(base: &Path, root: &Path) -> Vec<Finding> {
    let seeds = base.join(root).join(SEEDS);
    let sources: Vec<PathBuf> = children(&base.join(root))
        .into_iter()
        .filter(|d| d.join("lib.rs").is_file() || d.join("main.rs").is_file())
        .collect();
    let mut files = Vec::new();
    txt_files(&seeds, &mut files);
    files
        .into_iter()
        .filter_map(|file| {
            let rel = file.strip_prefix(&seeds).ok()?.with_extension("rs");
            if sources.iter().any(|s| s.join(&rel).is_file()) {
                return None;
            }
            let path = file
                .strip_prefix(base)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            Some(finding(path, &rel, moved_to(base, root, &rel)))
        })
        .collect()
}

/// Where the seed belongs when its module moved into its crate: the root's
/// `<m>/<rest>` is `crates/<m>/proptest-regressions/<rest>` once `crates/<m>/src/<rest>.rs` exists.
fn moved_to(base: &Path, root: &Path, rel: &Path) -> Option<String> {
    if !root.as_os_str().is_empty() {
        return None;
    }
    let mut parts = rel.iter();
    let m = parts.next()?.to_string_lossy().into_owned();
    let rest: PathBuf = parts.collect();
    let krate = Path::new(crate::modules::CRATES).join(&m);
    base.join(&krate)
        .join("src")
        .join(&rest)
        .is_file()
        .then(|| {
            let to = krate.join(SEEDS).join(rest.with_extension("txt"));
            to.to_string_lossy().replace('\\', "/")
        })
}

fn finding(path: String, rel: &Path, moved: Option<String>) -> Finding {
    let source = rel.to_string_lossy().replace('\\', "/");
    let fix = match moved {
        Some(to) => format!("its module moved: `git mv {path} {to}`"),
        None => "move it beside the crate whose test it saved, or delete it with that test".into(),
    };
    Finding {
        path,
        line: 1,
        rule: RULE,
        message: format!(
            "no test reads this proptest seed file: no `{source}` sits beside a `lib.rs` \
             or `main.rs` here, so its cases never replay; {fix}"
        ),
    }
}

/// The folders directly under `dir`, sorted; none when it is missing.
fn children(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    out.sort();
    out
}

/// Every `.txt` under `dir`, depth first, sorted.
fn txt_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            txt_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "txt") {
            out.push(p);
        }
    }
}

/// The rule over the repository at the working directory.
pub fn run(_: &Scan) -> Vec<Finding> {
    // `.`, not `""`: `read_dir("")` fails, and every seed would read as orphaned.
    check(Path::new("."))
}

#[cfg(test)]
#[path = "orphan_seeds_tests.rs"]
mod tests;
