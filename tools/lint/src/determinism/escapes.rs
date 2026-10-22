//! The locked lint escapes (#961): lints whose exceptions are counted, not merely
//! explained.
//!
//! Every `#[allow]`/`#[expect]` already needs a `reason` (clippy's
//! `allow_attributes_without_reason`). Two lints get more than that: the
//! function-length cap has exactly one sanctioned exception, `Renderer::assemble`,
//! and the sim's `unwrap_used` ban has none (docs/linting.md). This scan finds every
//! `allow`/`expect` attribute that names one of them and reports any beyond
//! [`ESCAPES`], so a second exception is a failed gate rather than a quiet line.
use crate::code::{code_of, Open};

/// The lints whose escapes are locked.
const LOCKED: &[&str] = &["clippy::too_many_lines", "clippy::unwrap_used"];

/// The sanctioned escapes: (file, lint). Each permits one attribute in that file.
const ESCAPES: &[(&str, &str)] = &[("src/render/setup/mod.rs", "clippy::too_many_lines")];

/// What the scan walks: the engine crate's own source, as clippy lints it.
pub const ROOTS: &[&str] = &["src", "tests", "fuzz", "build.rs"];

/// Every attribute in `content` that silences a locked lint without being one of
/// the sanctioned [`ESCAPES`], reported at the attribute's first line.
pub fn violations(path: &str, content: &str) -> Vec<String> {
    let mut sanctioned: Vec<&str> = ESCAPES
        .iter()
        .filter(|(p, _)| *p == path)
        .map(|(_, lint)| *lint)
        .collect();
    let mut out = Vec::new();
    for (line, attr) in attributes(content) {
        if !(attr.contains("allow(") || attr.contains("expect(")) {
            continue;
        }
        for lint in LOCKED.iter().filter(|l| super::names_lint(&attr, l)) {
            if let Some(i) = sanctioned.iter().position(|s| s == lint) {
                sanctioned.remove(i);
                continue;
            }
            out.push(format!(
                "ESCAPE {path}:{line} `{lint}` — this lint's exceptions are locked; \
                 fix the code instead (docs/linting.md)"
            ));
        }
    }
    out
}

/// Each `#[…]`/`#![…]` attribute in `content`, comments stripped and continuation
/// lines joined, with the 1-based line it starts on.
fn attributes(content: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut open: Option<(usize, String, i32)> = None;
    let mut literal = Open::None;
    for (i, raw) in content.lines().enumerate() {
        let mut brackets = 0i32;
        let code = code_of(raw, &mut literal, |_, c| match c {
            '[' => brackets += 1,
            ']' => brackets -= 1,
            _ => {}
        });
        let (start, mut text, mut depth) = match open.take() {
            Some(o) => o,
            None => {
                let trimmed = code.trim_start();
                if !(trimmed.starts_with("#[") || trimmed.starts_with("#![")) {
                    continue;
                }
                (i + 1, String::new(), 0)
            }
        };
        text.push_str(&code);
        depth += brackets;
        if depth > 0 {
            open = Some((start, text, depth));
        } else {
            out.push((start, text));
        }
    }
    out
}
