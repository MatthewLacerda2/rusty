use super::finding::Finding;
use super::scan::Scan;
use crate::repo::ls_files;
use std::fs;
use std::path::{Path, PathBuf};

pub const RULE: &str = "sorted";
const KEYWORDS: [&str; 7] = ["pub", "crate", "super", "self", "mod", "use", "fn"];
const MARKDOWN_ROOTS: [&str; 2] = ["docs", ".claude/skills"];

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

fn reason(line: &str) -> Option<&str> {
    let t = line.trim();
    let rest = t
        .strip_prefix("// sorted:")
        .or_else(|| t.strip_prefix("<!-- sorted:"))?;
    Some(rest.trim_end_matches("-->").trim())
}

fn head(key: &str) -> &str {
    key.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()
        .unwrap_or(key)
}

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

fn is_note(line: &str) -> bool {
    let t = line.trim_start();
    ["//", "#[", "<!--"].iter().any(|p| t.starts_with(p))
}

fn is_table_rule(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.contains('-') && t.chars().all(|c| "|-: ".contains(c))
}

fn key(line: &str) -> Option<String> {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let mut rest = line;
    loop {
        let start = rest.find(word)?;
        let end = rest[start..]
            .find(|c| !word(c))
            .map_or(rest.len(), |e| start + e);
        let hyphenated = rest[end..].starts_with('-');
        if hyphenated || !KEYWORDS.contains(&&rest[start..end]) {
            return Some(rest[start..].trim_end().to_string());
        }
        rest = &rest[end..];
    }
}

/// Tracked only: a gitignored note never reaches CI, so it must not fail the gate locally.
pub fn markdown(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut md = ls_files(root, &[], &["*.md"])?;
    md.retain(|p| p.components().count() == 1 || in_roots(p));
    Ok(md)
}

fn in_roots(path: &Path) -> bool {
    MARKDOWN_ROOTS.iter().any(|r| path.starts_with(r))
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    let mut found = scan.each_file(check);
    found.extend(markdown_findings(scan.root()));
    found
}

fn markdown_findings(root: &Path) -> Vec<Finding> {
    let problem = |path: String, message: String| Finding {
        path,
        line: 1,
        rule: RULE,
        message,
    };
    let paths = match markdown(root) {
        Ok(paths) => paths,
        Err(why) => {
            return vec![problem(
                ".".into(),
                format!("could not list the tracked Markdown: {why}"),
            )]
        }
    };
    let mut found = Vec::new();
    for path in paths {
        let shown = path.to_string_lossy().into_owned();
        match fs::read_to_string(root.join(&path)) {
            Ok(source) => found.extend(check(&shown, &source)),
            Err(why) => found.push(problem(shown, format!("could not read this file: {why}"))),
        }
    }
    found
}

#[cfg(test)]
#[path = "sorted_tests.rs"]
mod tests;
