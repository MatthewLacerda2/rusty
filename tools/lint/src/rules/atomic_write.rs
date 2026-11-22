use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "atomic-write";
const HOME: &str = "src/core/fs/";

fn imported_names(code: &[std::borrow::Cow<str>]) -> Vec<String> {
    let text = code.join("\n");
    let mut names = Vec::new();
    for stmt in text.split(';') {
        let Some(at) = stmt.find("use std::fs::") else {
            continue;
        };
        let tree = &stmt[at + "use std::fs::".len()..];
        for item in tree.split(['{', '}', ',']) {
            let mut words = item.split_whitespace();
            let path = words.next().unwrap_or("");
            if path != "rename" {
                continue;
            }
            match (words.next(), words.next()) {
                (Some("as"), Some(alias)) => names.push(alias.to_string()),
                _ => names.push("rename".to_string()),
            }
        }
    }
    names
}

fn calls_bare(code: &str, name: &str) -> bool {
    let call = format!("{name}(");
    code.match_indices(&call).any(|(i, _)| {
        let before = code[..i].chars().next_back();
        let ident = |c: char| c.is_alphanumeric() || c == '_';
        let declared = code[..i].trim_end().ends_with("fn");
        !before.is_some_and(|c| ident(c) || c == '.' || c == ':') && !declared
    })
}

pub fn check(path: &str, source: &str) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with("src/") || at.starts_with(HOME) {
        return Vec::new();
    }
    let lines = code_lines(source);
    let names = imported_names(&lines);
    lines
        .iter()
        .enumerate()
        .filter(|(_, code)| {
            code.contains("fs::rename") || names.iter().any(|n| calls_bare(code, n))
        })
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "write through `core::fs::write_atomic`, the one atomic write; \
                      a rename that moves rather than writes takes an inline allow"
                .to_string(),
        })
        .collect()
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "atomic_write_tests.rs"]
mod tests;
