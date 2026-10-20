//! `sorted`: a list marked order-free stays in order (#911).
//!
//! Two branches appending to the end of one list always collide on its last line;
//! entries inserted in order land on different lines, so most pairs merge clean. A
//! list whose order means nothing says so on the line above it, with the reason:
//!
//! ```text
//! // sorted: <why order is free>          (Rust)
//! <!-- sorted: <why order is free> -->    (Markdown)
//! ```
//!
//! The list runs from the line after the marker to the first blank line, or the
//! first line indented less than its first entry. An entry's key is the line from
//! its first word that is not a Rust keyword (`pub`, `crate`, `mod`, …) on, compared
//! ignoring case, so `pub(crate) mod a;` sorts as `a;` and `- [`A`](A.md)` as `A`.
//! Lines deeper than the first entry continue the one above; comment and attribute
//! lines, lines with no word, and a Markdown table's header (everything up to its
//! `|---|` row) are not entries. A marker with no reason is a finding.

use super::finding::Finding;
use super::scan::Scan;
use std::fs;
use std::path::Path;

pub const RULE: &str = "sorted";
/// Words skipped before an entry's key starts.
const KEYWORDS: [&str; 7] = ["pub", "crate", "super", "self", "mod", "use", "fn"];
/// Where the Markdown markers are looked for, beside the Rust files every rule scans.
const MARKDOWN_ROOTS: [&str; 2] = ["docs", ".claude/skills"];

/// Every out-of-order entry under a marker in `source`.
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    let lines: Vec<&str> = source.lines().collect();
    let finding = |line: usize, message: String| Finding {
        path: path.to_string(),
        line: line + 1,
        rule: RULE,
        message,
    };
    let mut found = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        let Some(why) = reason(line) else { continue };
        if why.is_empty() {
            found.push(finding(
                at,
                "a `sorted` marker needs a reason after it".into(),
            ));
            continue;
        }
        let mut prev: Option<String> = None;
        for (i, key) in entries(&lines, at + 1) {
            if let Some(p) = prev
                .as_ref()
                .filter(|p| key.to_lowercase() < p.to_lowercase())
            {
                let (k, p) = (head(&key), head(p));
                let msg = format!("`{k}` belongs before `{p}` in this sorted list");
                found.push(finding(i, msg));
            }
            prev = Some(key);
        }
    }
    found
}

/// The reason a marker line gives, `None` when `line` is no marker.
fn reason(line: &str) -> Option<&str> {
    let t = line.trim();
    let rest = t
        .strip_prefix("// sorted:")
        .or_else(|| t.strip_prefix("<!-- sorted:"))?;
    Some(rest.trim_end_matches("-->").trim())
}

/// A key's first word, which names the entry in a finding.
fn head(key: &str) -> &str {
    key.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()
        .unwrap_or(key)
}

/// The `(line index, key)` of each entry in the list starting at `from`.
fn entries(lines: &[&str], from: usize) -> Vec<(usize, String)> {
    let indent = |l: &str| l.len() - l.trim_start().len();
    let block: Vec<(usize, &str)> = lines[from..]
        .iter()
        .enumerate()
        .map(|(i, l)| (from + i, *l))
        .take_while(|(_, l)| !l.trim().is_empty())
        .collect();
    let header = block.iter().position(|(_, l)| is_table_rule(l));
    let block = &block[header.map_or(0, |h| h + 1)..];
    let Some(depth) = block.first().map(|(_, l)| indent(l)) else {
        return Vec::new();
    };
    block
        .iter()
        .take_while(|(_, l)| indent(l) >= depth)
        .filter(|(_, l)| indent(l) == depth && !is_note(l))
        .filter_map(|(i, l)| Some((*i, key(l)?)))
        .collect()
}

/// A comment or attribute line, which annotates an entry rather than being one.
fn is_note(line: &str) -> bool {
    let t = line.trim_start();
    ["//", "#[", "<!--"].iter().any(|p| t.starts_with(p))
}

/// A Markdown table's `|---|---|` row.
fn is_table_rule(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.contains('-') && t.chars().all(|c| "|-: ".contains(c))
}

/// The line from its first non-keyword word on; `None` when it has no word.
fn key(line: &str) -> Option<String> {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let mut rest = line;
    loop {
        let start = rest.find(word)?;
        let end = rest[start..]
            .find(|c| !word(c))
            .map_or(rest.len(), |e| start + e);
        if !KEYWORDS.contains(&&rest[start..end]) {
            return Some(rest[start..].trim_end().to_string());
        }
        rest = &rest[end..];
    }
}

/// The Markdown files under [`MARKDOWN_ROOTS`] and at the top of `root`, relative to it.
pub fn markdown(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut dirs: Vec<_> = MARKDOWN_ROOTS.iter().map(|d| root.join(d)).collect();
    let top = fs::read_dir(root).into_iter().flatten().flatten();
    out.extend(top.map(|e| e.path()).filter(|p| p.is_file()));
    while let Some(dir) = dirs.pop() {
        for path in fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
        {
            if path.is_dir() {
                dirs.push(path);
            } else {
                out.push(path);
            }
        }
    }
    let mut md: Vec<String> = out
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .filter_map(|p| p.strip_prefix(root).ok())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    md.sort();
    md
}

/// The rule over every scanned file and the Markdown under the working directory.
pub fn run(scan: &Scan) -> Vec<Finding> {
    let mut found = scan.each_file(check);
    for path in markdown(Path::new(".")) {
        found.extend(check(&path, &fs::read_to_string(&path).unwrap_or_default()));
    }
    found
}

#[cfg(test)]
#[path = "sorted_tests.rs"]
mod tests;
