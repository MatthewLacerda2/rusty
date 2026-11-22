use super::finding::Finding;
use super::scan::Scan;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub const RULE: &str = "prefix-group";
const MIN_GROUP: usize = 3;
pub const AIM_FILES_PER_DIR: usize = 10;

const EXEMPT: &[(&str, &str)] = &[("src/script/api", "one file per Lua namespace")];

fn stem(file: &str) -> Option<&str> {
    let s = file.strip_suffix(".rs")?;
    if matches!(s, "mod" | "main" | "lib") {
        return None;
    }
    Some(s.strip_suffix("_tests").unwrap_or(s))
}

fn word(stem: &str) -> &str {
    stem.split(['_', '-']).next().unwrap_or(stem)
}

pub fn groups<'a>(files: &[&'a str]) -> Vec<(&'a str, Vec<&'a str>)> {
    let mut by_word: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for s in files.iter().filter_map(|f| stem(f)) {
        let stems = by_word.entry(word(s)).or_default();
        if !stems.contains(&s) {
            stems.push(s);
        }
    }
    by_word
        .into_iter()
        .filter(|(_, stems)| stems.len() >= MIN_GROUP)
        .map(|(w, mut stems)| {
            stems.sort_unstable();
            (w, stems)
        })
        .collect()
}

pub fn anchor(owner: &str, files: &[&str]) -> Option<usize> {
    let mods: Vec<&str> = files.iter().filter_map(|f| f.strip_suffix(".rs")).collect();
    owner
        .lines()
        .position(|l| {
            let l = l.trim();
            let l = l.strip_prefix("pub(crate) ").unwrap_or(l);
            let l = l.strip_prefix("pub ").unwrap_or(l);
            l.strip_prefix("mod ")
                .and_then(|m| m.strip_suffix(';'))
                .is_some_and(|m| mods.contains(&m))
        })
        .map(|i| i + 1)
}

pub fn owner_of(base: &Path, dir: &str) -> Option<String> {
    ["mod.rs", "main.rs", "lib.rs"]
        .iter()
        .map(|f| format!("{dir}/{f}"))
        .chain(std::iter::once(format!("{dir}.rs")))
        .find(|p| base.join(p).is_file())
}

pub fn check_dir(base: &Path, dir: &str, files: &[&str]) -> Vec<Finding> {
    if EXEMPT
        .iter()
        .any(|(d, _)| *d == crate::modules::module_path(dir))
    {
        return Vec::new();
    }
    let owner =
        owner_of(base, dir).map(|p| (fs::read_to_string(base.join(&p)).unwrap_or_default(), p));
    groups(files)
        .into_iter()
        .map(|(w, stems)| {
            let members: Vec<&str> = files
                .iter()
                .copied()
                .filter(|f| stem(f).is_some_and(|s| stems.contains(&s)))
                .collect();
            let (path, line) = owner
                .as_ref()
                .and_then(|(src, p)| Some((p.clone(), anchor(src, &members)?)))
                .unwrap_or_else(|| (format!("{dir}/{}.rs", stems[0]), 1));
            Finding {
                path,
                line,
                rule: RULE,
                message: format!(
                    "{} files in {dir}/ share the prefix `{w}` ({}): move them into `{w}/` and drop the prefix",
                    stems.len(),
                    stems.join(", ")
                ),
            }
        })
        .collect()
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    let dirs = scan.dirs();
    dirs.iter()
        .flat_map(|(d, names)| check_dir(&scan.base, d, names))
        .collect()
}

#[cfg(test)]
#[path = "prefix_tests.rs"]
mod tests;
