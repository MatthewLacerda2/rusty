//! What every rule reads (#911): the scanned `.rs` files and their text, read once.
//! A rule is `fn(&Scan) -> Vec<Finding>`, one row of `rules::RULES`.

use super::finding::Finding;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub struct Scan {
    /// Every `.rs` file under [`crate::size::SCAN_ROOTS`], `/`-separated.
    pub files: Vec<String>,
    sources: BTreeMap<String, String>,
    /// The layer table's sim modules (`src/<module>`).
    pub sim_dirs: Vec<String>,
}

impl Scan {
    /// Walk [`crate::size::SCAN_ROOTS`] and read every file once.
    pub fn load() -> Self {
        let files: Vec<String> = crate::size::SCAN_ROOTS
            .iter()
            .flat_map(|root| crate::size::scan_dir(Path::new(root)))
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        let sources = files
            .iter()
            .map(|p| (p.clone(), fs::read_to_string(p).unwrap_or_default()))
            .collect();
        Self {
            files,
            sources,
            sim_dirs: crate::layers::sim_dirs(),
        }
    }

    /// The text of a scanned file; empty for any other path.
    pub fn source(&self, path: &str) -> &str {
        self.sources.get(path).map_or("", String::as_str)
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
