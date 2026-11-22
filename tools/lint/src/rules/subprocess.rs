use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "subprocess";

pub fn check(path: &str, source: &str) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with("src/") {
        return Vec::new();
    }
    code_lines(source)
        .iter()
        .enumerate()
        .filter(|(_, code)| code.contains("process::") && has_word(code.as_ref(), "Command"))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message:
                "the engine starts no other program but Blender, while authoring (CLAUDE.md, *One format inside*)"
                    .to_string(),
        })
        .collect()
}

fn has_word(code: &str, word: &str) -> bool {
    let ident = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    code.match_indices(word).any(|(at, _)| {
        !ident(code[..at].chars().next_back()) && !ident(code[at + word.len()..].chars().next())
    })
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "subprocess_tests.rs"]
mod tests;
