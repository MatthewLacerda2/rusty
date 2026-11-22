//! so with `// lint: allow(asset-write) <why>`, which is also the note on why it

use super::finding::Finding;
use super::scan::Scan;
use crate::code::{bare_lines, is_ident};

pub const RULE: &str = "asset-write";
const HOME: &str = "src/core/fs/";

fn writes(code: &str) -> bool {
    let saves = code.match_indices(".save(").any(|(at, _)| {
        let arg = &code[at + ".save(".len()..];
        !arg.trim_start().starts_with(')')
    });
    let atomic = code.match_indices("write_atomic(").any(|(at, _)| {
        let before = &code[..at];
        !before.chars().next_back().is_some_and(is_ident) && !before.ends_with("fn ")
    });
    code.contains("fs::write(") || code.contains("File::create(") || saves || atomic
}

fn notes_after(code: &[String], from: usize) -> bool {
    for line in &code[from..] {
        if line.contains("note_written(") {
            return true;
        }
    }
    false
}

fn starts_fn(line: &str) -> bool {
    let mut l = line.trim_start();
    if let Some(rest) = l.strip_prefix("pub") {
        l = match rest.strip_prefix('(') {
            Some(scoped) => scoped.split_once(')').map_or(rest, |(_, r)| r),
            None => rest,
        };
        if !l.starts_with(' ') {
            return false;
        }
        l = l.trim_start();
    }
    loop {
        let next = ["const ", "async ", "unsafe ", "extern "]
            .iter()
            .find_map(|q| l.strip_prefix(q));
        match next {
            Some(rest) => l = rest.trim_start(),
            None => return l.starts_with("fn "),
        }
    }
}

fn fn_end(code: &[String], from: usize) -> usize {
    (from + 1..code.len())
        .find(|&i| starts_fn(&code[i]))
        .unwrap_or(code.len())
}

pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with("src/") || at.starts_with(HOME) {
        return Vec::new();
    }
    let code = bare_lines(source);
    (0..code.len())
        .filter(|&i| !test.get(i).copied().unwrap_or(false) && writes(&code[i]))
        .filter(|&i| !notes_after(&code[..fn_end(&code, i)], i))
        .map(|i| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "write through `core::fs::written::write`, or call `note_written` after; \
                      a file no path-keyed cache reads takes an inline allow saying so"
                .to_string(),
        })
        .collect()
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.files
        .iter()
        .flat_map(|p| check(p, scan.source(p), scan.test_lines(p)))
        .collect()
}

#[cfg(test)]
#[path = "asset_write_tests.rs"]
mod tests;
