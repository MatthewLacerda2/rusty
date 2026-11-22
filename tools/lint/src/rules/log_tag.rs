//! message that is not a literal says why with `// lint: allow(log-tag) <why>`.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "log-tag";

const MACROS: &[&str] = &["warn!", "error!", "log!"];

const SPREAD: usize = 6;

pub fn check(path: &str, source: &str) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with("src/") {
        return Vec::new();
    }
    let bare = source.contains("use log::");
    let lines = code_lines(source);
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        for (name, at) in calls(line, bare) {
            let end = (i + 1 + SPREAD).min(lines.len());
            let args = format!("{} {}", &line[at..], lines[i + 1..end].join(" "));
            if !passes(name, &args) {
                found.push(Finding {
                    path: path.to_string(),
                    line: i + 1,
                    rule: RULE,
                    message: "an engine warning opens with its module, `[Asset] …`".to_string(),
                });
            }
        }
    }
    found
}

fn calls(line: &str, bare: bool) -> Vec<(&'static str, usize)> {
    let mut out = Vec::new();
    for &name in MACROS {
        for (at, _) in line.match_indices(name) {
            let before = &line[..at];
            let qualified = before.ends_with("log::");
            let ident = |c: char| c.is_alphanumeric() || c == '_' || c == ':';
            let unqualified = !before.ends_with(ident);
            if !(qualified || bare && unqualified) {
                continue;
            }
            let after = &line[at + name.len()..];
            let open = after.len() - after.trim_start().len();
            if after[open..].starts_with('(') {
                out.push((name, at + name.len() + open + 1));
            }
        }
    }
    out
}

fn passes(name: &str, args: &str) -> bool {
    let mut rest = skip_target(args.trim_start());
    if name == "log!" {
        let Some((level, after)) = rest.split_once(',') else {
            return true;
        };
        let level = level.trim();
        if !(level.ends_with("Level::Warn") || level.ends_with("Level::Error")) {
            return true;
        }
        rest = after.trim_start();
    }
    let literal = match rest.strip_prefix('r') {
        Some(raw) => raw.trim_start_matches('#'),
        None => rest,
    };
    literal.starts_with("\"[")
}

fn skip_target(args: &str) -> &str {
    let Some(after) = args.strip_prefix("target:") else {
        return args;
    };
    let after = after.trim_start().strip_prefix('"').unwrap_or(after);
    let rest = after.split_once('"').map_or("", |(_, r)| r).trim_start();
    rest.strip_prefix(',').unwrap_or(rest).trim_start()
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "log_tag_tests.rs"]
mod tests;
