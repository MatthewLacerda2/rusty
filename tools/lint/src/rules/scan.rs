//! What every rule reads (#911): the scanned `.rs` files and their text, read once.
//! A rule is `fn(&Scan) -> Vec<Finding>`, one row of `rules::RULES`.

use super::finding::Finding;
use super::test_code;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub struct Scan {
    /// Every `.rs` file under [`crate::size::SCAN_ROOTS`], `/`-separated.
    pub files: Vec<String>,
    sources: BTreeMap<String, String>,
    /// For each file, whether each line is test code (`test_code.rs`), worked out
    /// once for every rule that asks.
    test_lines: BTreeMap<String, Vec<bool>>,
    /// The layer table's sim modules (`src/<module>`).
    pub sim_dirs: Vec<String>,
}

impl Scan {
    /// Walk [`crate::size::SCAN_ROOTS`] and read every file once.
    pub fn load() -> Self {
        Self::at(Path::new(""))
    }

    /// [`Scan::load`] for the files of the repository at `base`; paths stay
    /// relative to it. `sim_dirs` still comes from this build's layer table, not
    /// from `base`: the table is code, compiled in.
    pub fn at(base: &Path) -> Self {
        let found: Vec<PathBuf> = crate::size::SCAN_ROOTS
            .iter()
            .flat_map(|root| crate::size::scan_dir(&base.join(root)))
            .collect();
        let relative = |p: &Path| {
            let p = p.strip_prefix(base).unwrap_or(p);
            p.to_string_lossy().replace('\\', "/")
        };
        let files: Vec<String> = found.iter().map(|p| relative(p)).collect();
        let sources: BTreeMap<String, String> = files
            .iter()
            .map(|p| {
                (
                    p.clone(),
                    fs::read_to_string(base.join(p)).unwrap_or_default(),
                )
            })
            .collect();
        let test_only: BTreeSet<String> = crate::layers::test_only_files(&found, |p| {
            sources.get(&relative(p)).map(String::as_str)
        })
        .iter()
        .map(|p| relative(p))
        .collect();
        let test_lines = sources
            .iter()
            .map(|(p, text)| {
                let lines = test_code::test_lines(p, text, test_only.contains(p));
                (p.clone(), lines)
            })
            .collect();
        Self {
            files,
            sources,
            test_lines,
            sim_dirs: crate::layers::sim_dirs(),
        }
    }

    /// The text of a scanned file; empty for any other path.
    pub fn source(&self, path: &str) -> &str {
        self.sources.get(path).map_or("", String::as_str)
    }

    /// For each line of a scanned file, whether it is test code (`test_code.rs`).
    pub fn test_lines(&self, path: &str) -> &[bool] {
        self.test_lines.get(path).map_or(&[], Vec::as_slice)
    }

    /// `check` over every scanned file, findings in file order.
    pub fn each_file(&self, check: impl Fn(&str, &str) -> Vec<Finding>) -> Vec<Finding> {
        self.files
            .iter()
            .flat_map(|p| check(p, self.source(p)))
            .collect()
    }

    /// File names grouped by their directory.
    pub fn dirs(&self) -> BTreeMap<&str, Vec<&str>> {
        let mut dirs: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for f in &self.files {
            if let Some((dir, name)) = f.rsplit_once('/') {
                dirs.entry(dir).or_default().push(name);
            }
        }
        dirs
    }
}

#[cfg(test)]
#[path = "scan_tests.rs"]
mod tests;
