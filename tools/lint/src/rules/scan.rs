use super::finding::Finding;
use super::test_code;
use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub struct Package<'a> {
    pub text: String,
    pub files: Vec<&'a str>,
}

pub fn names(text: &str, dep: &str) -> bool {
    let key = dep.replace('-', "_");
    let mut named = crate::layers::manifest_dependencies(text);
    named.extend(crate::layers::manifest_dev_dependencies(text));
    named.contains(&key)
}

pub struct Scan {
    pub base: PathBuf,
    pub files: Vec<String>,
    sources: BTreeMap<String, String>,
    test_lines: BTreeMap<String, Vec<bool>>,
    pub sim_dirs: Vec<String>,
    shaders: OnceCell<Vec<(String, String)>>,
}

impl Scan {
    pub fn at(base: &Path) -> Self {
        let found: Vec<PathBuf> = crate::size::SCAN_ROOTS
            .iter()
            .flat_map(|root| crate::size::scan_dir(&base.join(root)))
            .collect();
        let relative = |p: &Path| {
            let p = p.strip_prefix(base).unwrap_or(p);
            p.to_string_lossy().into_owned()
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
            base: base.to_path_buf(),
            files,
            sources,
            test_lines,
            sim_dirs: crate::layers::sim_dirs(),
            shaders: OnceCell::new(),
        }
    }

    pub fn root(&self) -> &Path {
        if self.base.as_os_str().is_empty() {
            Path::new(".")
        } else {
            &self.base
        }
    }

    pub fn shaders(&self) -> &[(String, String)] {
        self.shaders.get_or_init(|| {
            crate::size::shaders_in(&self.base.join("engine"))
                .iter()
                .map(|p| {
                    let rel = p.strip_prefix(&self.base).unwrap_or(p);
                    let text = fs::read_to_string(p).unwrap_or_default();
                    (rel.to_string_lossy().into_owned(), text)
                })
                .collect()
        })
    }

    pub fn source(&self, path: &str) -> &str {
        self.sources.get(path).map_or("", String::as_str)
    }

    pub fn test_lines(&self, path: &str) -> &[bool] {
        self.test_lines.get(path).map_or(&[], Vec::as_slice)
    }

    pub fn each_file(&self, check: impl Fn(&str, &str) -> Vec<Finding>) -> Vec<Finding> {
        self.files
            .iter()
            .flat_map(|p| check(p, self.source(p)))
            .collect()
    }

    pub fn packages(&self) -> Vec<Package<'_>> {
        let members = crate::modules::member_manifests(&self.base);
        let members = members
            .iter()
            .map(|m| m.strip_prefix(&self.base).unwrap_or(m))
            .map(|m| m.to_string_lossy().replace('\\', "/"));
        std::iter::once(String::from("Cargo.toml"))
            .chain(members)
            .map(|manifest| {
                let root = manifest.trim_end_matches("Cargo.toml");
                let dirs = [format!("{root}src/"), format!("{root}tests/")];
                let inside = |f: &&String| dirs.iter().any(|d| f.starts_with(d.as_str()));
                let files = self
                    .files
                    .iter()
                    .filter(inside)
                    .map(String::as_str)
                    .collect();
                let text = fs::read_to_string(self.base.join(&manifest)).unwrap_or_default();
                Package { text, files }
            })
            .collect()
    }

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
