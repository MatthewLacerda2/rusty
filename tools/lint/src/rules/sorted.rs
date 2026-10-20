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
//! first line indented less than its first entry. An entry's key is the rest of its
//! line from the first word that is not a Rust keyword (`pub`, `crate`, `mod`, …),
//! trailing space trimmed and compared ignoring case: `pub(crate) mod a;` sorts as
//! `a;`, `- [`A`](A.md)` as ``A`](A.md)``. Lines deeper than the first entry
//! continue the one above; comment and attribute lines, lines with no word, and a
//! Markdown table's header (a `|` row and its `|---|` rule, right under the marker)
//! are not entries. A marker is itself a finding when it gives no reason, does not
//! fit on one line, or governs no entries: nothing under it, or a first entry
//! indented deeper than the marker.

use super::finding::Finding;
use super::scan::Scan;
use std::fs;
use std::path::Path;
use std::process::Command;

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
        if let Some(problem) = malformed(line, why, &lines, at) {
            found.push(finding(at, problem.into()));
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

/// What is wrong with the marker at `at`, which gives `why`, if anything.
fn malformed(line: &str, why: &str, lines: &[&str], at: usize) -> Option<&'static str> {
    if why.is_empty() {
        return Some("a `sorted` marker needs a reason after it");
    }
    if line.trim_start().starts_with("<!--") && !line.trim_end().ends_with("-->") {
        return Some("a `sorted` marker must fit on one line");
    }
    match entries(lines, at + 1).first() {
        None => Some("this `sorted` marker governs no entries: start the list right under it"),
        Some((i, _)) if indent(lines[*i]) > indent(line) => {
            Some("this `sorted` list starts deeper than its marker; indent them alike")
        }
        Some(_) => None,
    }
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
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
    let block: Vec<(usize, &str)> = lines[from..]
        .iter()
        .enumerate()
        .map(|(i, l)| (from + i, *l))
        .take_while(|(_, l)| !l.trim().is_empty())
        .collect();
    let header = block
        .iter()
        .take_while(|(_, l)| l.trim_start().starts_with('|'))
        .position(|(_, l)| is_table_rule(l));
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

/// The tracked Markdown files under [`MARKDOWN_ROOTS`] and at the top of the repo at
/// `root`, relative to it. Only tracked ones: a gitignored note (the operator's
/// `auxmd.md`) never reaches CI, so it must not fail the gate locally either.
pub fn markdown(root: &Path) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z", "--", "*.md"])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let mut md: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|p| !p.is_empty() && (!p.contains('/') || in_roots(p)))
        .map(str::to_string)
        .collect();
    md.sort();
    Ok(md)
}

fn in_roots(path: &str) -> bool {
    MARKDOWN_ROOTS
        .iter()
        .any(|r| path.strip_prefix(r).is_some_and(|p| p.starts_with('/')))
}

/// The rule over every scanned file and the Markdown under the working directory.
pub fn run(scan: &Scan) -> Vec<Finding> {
    let mut found = scan.each_file(check);
    match markdown(Path::new(".")) {
        Ok(paths) => {
            for path in paths {
                found.extend(check(&path, &fs::read_to_string(&path).unwrap_or_default()));
            }
        }
        Err(why) => found.push(Finding {
            path: ".".into(),
            line: 1,
            rule: RULE,
            message: format!("could not list the tracked Markdown: {why}"),
        }),
    }
    found
}

#[cfg(test)]
#[path = "sorted_tests.rs"]
mod tests;
