use super::finding::Finding;
use super::scan::{names, Scan};
use crate::code::{bare_lines, is_ident};
use crate::layers::flatten_use;

pub const RULE: &str = "one-logger";

const CRATE: &str = "rusty_test_support";

const GLOB: &str = "__glob__";

pub fn check(path: &str, manifest: &str, source: &str) -> Vec<Finding> {
    match names(manifest, "rusty-logging") {
        true => found(path, source),
        false => Vec::new(),
    }
}

fn found(path: &str, source: &str) -> Vec<Finding> {
    let code = bare_lines(source).join("\n");
    let line = |at: usize| code[..at].matches('\n').count() + 1;
    let items = uses(&code);
    let mut roots = vec![CRATE.to_string()];
    for (_, tree) in &items {
        let paths = flatten_use(tree);
        roots.extend(
            paths
                .into_iter()
                .filter(|(p, _)| p == &[CRATE])
                .map(|(_, b)| b),
        );
    }
    let mut lines: Vec<usize> = items
        .iter()
        .filter(|(_, tree)| {
            flatten_use(&tree.replace('*', GLOB))
                .iter()
                .any(|(p, _)| brings_logs(p, &roots))
        })
        .map(|&(at, _)| line(at))
        .collect();
    for root in &roots {
        lines.extend(paths_to(&code, &format!("{root}::logs")).map(line));
    }
    lines.sort_unstable();
    lines.dedup();
    lines.into_iter().map(|line| finding(path, line)).collect()
}

fn brings_logs(path: &[String], roots: &[String]) -> bool {
    matches!(path, [root, next, ..] if roots.contains(root) && (next == "logs" || next == GLOB))
}

fn uses(code: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for at in paths_to(code, "use") {
        let rest = &code[at + 3..];
        if rest.starts_with(char::is_whitespace) {
            let end = rest.find(';').unwrap_or(rest.len());
            out.push((at, rest[..end].trim().trim_start_matches("::").to_string()));
        }
    }
    out
}

fn paths_to<'a>(code: &'a str, path: &'a str) -> impl Iterator<Item = usize> + 'a {
    code.match_indices(path)
        .map(|(at, _)| at)
        .filter(move |&at| {
            let before = code[..at].chars().next_back();
            let after = code[at + path.len()..].chars().next();
            !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
        })
}

fn finding(path: &str, line: usize) -> Finding {
    Finding {
        path: path.to_string(),
        line,
        rule: RULE,
        message: "this package names `rusty-logging`, so its tests read warnings through \
                  `rusty_logging::init_for_tests` and `rusty_logging::forward`: \
                  `rusty_test_support::logs` installs a second logger, and under plain \
                  `cargo test` whichever comes second captures nothing"
            .to_string(),
    }
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.packages()
        .iter()
        .flat_map(|p| {
            p.files
                .iter()
                .flat_map(|f| check(f, &p.text, scan.source(f)))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
#[path = "one_logger_tests.rs"]
mod tests;
