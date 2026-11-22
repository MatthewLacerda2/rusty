use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;
use std::path::Path;

pub const RULE: &str = "gpu-default-only";

pub fn check_file(path: &str, source: &str, test: &[bool], crates: &[String]) -> Vec<Finding> {
    let Some(m) = crate::modules::crate_module(path) else {
        return Vec::new();
    };
    if !crates.iter().any(|c| c == m) || !crate::modules::module_path(path).starts_with("src/") {
        return Vec::new();
    }
    let lines = code_lines(source);
    (0..lines.len())
        .filter(|&i| {
            let test = test.get(i).copied().unwrap_or(false);
            (!test && value_splits(&lines[i])) || attribute_splits(&attribute_at(&lines, i))
        })
        .map(|i| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: format!(
                "code here exists only without `dev`, and no GPU test draws it: the default \
                 suite leaves the gpu group to the dev one (docs/testing.md, *Which suite runs \
                 the GPU tests*). Keep it the same under both feature sets, or add rusty-{m}'s \
                 GPU tests back to the default pass in `make test` and ci.yml together \
                 (`-E 'not group(gpu*) | (group(gpu*) & package(rusty-{m}))'`); a switch no \
                 rendered path reads, or a test that is not a GPU one, takes an inline allow \
                 saying why"
            ),
        })
        .collect()
}

fn attribute_at(lines: &[std::borrow::Cow<'_, str>], i: usize) -> String {
    if !lines[i].trim_start().starts_with('#') {
        return String::new();
    }
    let mut joined = String::new();
    let mut depth = 0i32;
    for line in &lines[i..] {
        joined.push_str(line);
        joined.push(' ');
        depth += line.matches('[').count() as i32 - line.matches(']').count() as i32;
        if depth <= 0 {
            break;
        }
    }
    joined
}

fn value_splits(code: &str) -> bool {
    predicates(code, "cfg!(").any(names_dev)
}

fn attribute_splits(attribute: &str) -> bool {
    (predicates(attribute, "cfg(").chain(predicates(attribute, "cfg_attr(")))
        .any(|p| predicates(p, "not(").any(names_dev))
}

fn predicates<'a>(code: &'a str, open: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    code.match_indices(open)
        .filter(move |(at, _)| {
            !code[..*at]
                .chars()
                .next_back()
                .is_some_and(crate::code::is_ident)
        })
        .map(move |(at, _)| {
            let rest = &code[at + open.len()..];
            let mut depth = 1;
            let end = rest
                .char_indices()
                .find(|&(_, c)| {
                    depth += match c {
                        '(' => 1,
                        ')' => -1,
                        _ => 0,
                    };
                    depth == 0
                })
                .map_or(rest.len(), |(i, _)| i);
            &rest[..end]
        })
}

fn names_dev(predicate: &str) -> bool {
    let bare: String = predicate.chars().filter(|c| !c.is_whitespace()).collect();
    let both = bare.starts_with("any(") && bare.contains("feature=\"editor\"");
    bare.contains("feature=\"dev\"") && !both
}

pub fn scope(base: &Path, gpu_tests: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut crates = render_path(base);
    for m in gpu_tests {
        if !crates.contains(&m) {
            crates.push(m);
        }
    }
    crates
}

pub fn render_path(base: &Path) -> Vec<String> {
    let mut crates = vec!["render".to_string()];
    let mut i = 0;
    while i < crates.len() {
        let manifest = base.join("crates").join(&crates[i]).join("Cargo.toml");
        for m in dev_features(&std::fs::read_to_string(manifest).unwrap_or_default()) {
            if !crates.contains(&m) {
                crates.push(m);
            }
        }
        i += 1;
    }
    crates
}

fn dev_features(manifest: &str) -> Vec<String> {
    let Some(at) = manifest
        .lines()
        .position(|l| l.trim_start().starts_with("dev ") || l.trim_start().starts_with("dev="))
    else {
        return Vec::new();
    };
    let mut array = String::new();
    for line in manifest.lines().skip(at) {
        array.push_str(line);
        if line.contains(']') {
            break;
        }
    }
    array
        .split('"')
        .filter_map(|s| s.strip_prefix("rusty-")?.strip_suffix("/dev"))
        .map(|s| s.trim_end_matches('?').to_string())
        .collect()
}

fn declares_gpu_test(path: &str, source: &str, test: &[bool]) -> Option<String> {
    let m = crate::modules::crate_module(path)?;
    let declares = code_lines(source)
        .iter()
        .enumerate()
        .any(|(i, code)| test.get(i).copied().unwrap_or(false) && code.contains("fn gpu_"));
    (declares && crate::modules::module_path(path).starts_with("src/")).then(|| m.to_string())
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    let gpu = scan
        .files
        .iter()
        .filter_map(|p| declares_gpu_test(p, scan.source(p), scan.test_lines(p)));
    let crates = scope(&scan.base, gpu);
    scan.each_file(|p, s| check_file(p, s, scan.test_lines(p), &crates))
}

#[cfg(test)]
#[path = "gpu_default_only_tests.rs"]
mod tests;
