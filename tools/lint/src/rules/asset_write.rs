//! `asset-write`: an engine write of a file a path-keyed cache reads says so (#1220).
//!
//! `core::fs::written` is the one log of files the engine (re)wrote, so the
//! renderer's textures and cubemaps and the audio `ClipCache` follow a re-bake
//! (#689, #1208). A baker that writes with `std::fs::write` and tells nobody
//! brings the stale cache back. So a non-test line of `src/` (outside
//! `src/core/fs/`) whose code calls `fs::write(`, `File::create(` or `.save(` is a
//! finding unless the rest of its function calls `note_written` (`written::write`
//! is neither of those calls, so it never matches). A write no path-keyed cache
//! reads (a scene, prefab, project, settings, ledger, harness or bench file) says
//! so with `// lint: allow(asset-write) <why>`, which is also the note on why it
//! does not notify. A heuristic over text: `.save(` with an argument also matches
//! a project type's own `save(path)`, whose inner write carries the allow; the
//! caller takes one too. `tests/` and `tools/` are not the engine and are not scanned.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "asset-write";
const HOME: &str = "src/core/fs/";

/// Does `code` write a file: `fs::write(`, `File::create(` or `.save(<arg>)`?
fn writes(code: &str) -> bool {
    let saves = code.match_indices(".save(").any(|(at, _)| {
        let arg = &code[at + ".save(".len()..];
        !arg.trim_start().starts_with(')')
    });
    code.contains("fs::write(") || code.contains("File::create(") || saves
}

/// Does the function holding line `from` call `note_written` at or after it?
fn notes_after(code: &[std::borrow::Cow<str>], from: usize) -> bool {
    for line in &code[from..] {
        if line.contains("note_written(") {
            return true;
        }
    }
    false
}

/// The 0-based end of the function holding `from`: the line before the next `fn`.
fn fn_end(code: &[std::borrow::Cow<str>], from: usize) -> usize {
    let is_fn = |l: &str| {
        let l = l.trim_start();
        let l = l
            .strip_prefix("pub(crate) ")
            .or(l.strip_prefix("pub "))
            .unwrap_or(l);
        l.starts_with("fn ")
    };
    (from + 1..code.len())
        .find(|&i| is_fn(&code[i]))
        .unwrap_or(code.len())
}

/// Every non-test line of an engine source that writes a file with no
/// `note_written` after it in its function. `test[i]` says line `i` is test code.
pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    if !path.starts_with("src/") || path.starts_with(HOME) {
        return Vec::new();
    }
    let code = code_lines(source);
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
