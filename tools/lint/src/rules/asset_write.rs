//! `asset-write`: an engine write of a file a path-keyed cache reads says so (#1220).
//!
//! `core::fs::written` is the one log of files the engine (re)wrote, so the
//! renderer's textures and cubemaps and the audio `ClipCache` follow a re-bake
//! (#689, #1208). A baker that writes with `std::fs::write` and tells nobody
//! brings the stale cache back. So a non-test line of `src/` (outside
//! `src/core/fs/`) whose code (outside string literals) calls `fs::write(`,
//! `File::create(`, `.save(` or any `write_atomic(` (#2005) is a finding unless the rest of its function
//! calls `note_written` (`written::write` is none of those calls, so it never
//! matches). A write no path-keyed cache
//! reads (a scene, prefab, project, settings, ledger, harness or bench file) says
//! so with `// lint: allow(asset-write) <why>`, which is also the note on why it
//! does not notify. A heuristic over text: `.save(` with an argument also matches
//! a project type's own `save(path)`, whose inner write carries the allow; the
//! caller takes one too. `tests/` and `tools/` are not the engine and are not scanned.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::{bare_lines, is_ident};

pub const RULE: &str = "asset-write";
const HOME: &str = "src/core/fs/";

/// Does `code` write a file: `fs::write(`, `File::create(`, `.save(<arg>)` or a
/// call of any `write_atomic(` (a whole identifier: `try_write_atomic(` is not
/// one, and its definition, `fn write_atomic(`, is not a write)? `code` has its
/// string literals blanked ([`bare_lines`]), so a string naming a call is none.
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

/// Does the function holding line `from` call `note_written` at or after it?
fn notes_after(code: &[String], from: usize) -> bool {
    for line in &code[from..] {
        if line.contains("note_written(") {
            return true;
        }
    }
    false
}

/// Does `line` start a function: `fn `, after any visibility (`pub`, `pub(…)`)
/// and qualifiers (`const`, `async`, `unsafe`, `extern "…"`)? `line` is a
/// [`bare_lines`] line.
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
        // `extern "C"`: the ABI string is blanked to spaces, as every literal is.
        let next = ["const ", "async ", "unsafe ", "extern "]
            .iter()
            .find_map(|q| l.strip_prefix(q));
        match next {
            Some(rest) => l = rest.trim_start(),
            None => return l.starts_with("fn "),
        }
    }
}

/// The 0-based end of the function holding `from`: the line before the next `fn`.
fn fn_end(code: &[String], from: usize) -> usize {
    (from + 1..code.len())
        .find(|&i| starts_fn(&code[i]))
        .unwrap_or(code.len())
}

/// Every non-test line of an engine source that writes a file with no
/// `note_written` after it in its function. `test[i]` says line `i` is test code.
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

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.files
        .iter()
        .flat_map(|p| check(p, scan.source(p), scan.test_lines(p)))
        .collect()
}

#[cfg(test)]
#[path = "asset_write_tests.rs"]
mod tests;
