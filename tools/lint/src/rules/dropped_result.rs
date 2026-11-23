use super::finding::Finding;
use super::scan::Scan;
use crate::code::{code_lines, is_ident};

pub const RULE: &str = "dropped-result";
/// The editor and the shell's editor frontend, as module paths.
const REACHED: &[&str] = &["src/editor/", "src/shell/editor/"];
/// How many lines a statement may run before the scan gives it up.
const MAX_STATEMENT: usize = 24;

pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !REACHED.iter().any(|r| at.starts_with(r)) {
        return Vec::new();
    }
    let stripped = code_lines(source);
    let lines: Vec<&str> = stripped.iter().map(|l| l.trim()).collect();
    let live = |i: usize| !test.get(i).copied().unwrap_or(false);
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate().filter(|&(i, _)| live(i)) {
        let dropped = match line.strip_prefix("let _ =") {
            Some(rest) => calls(&statement_from(&lines[i..], rest)),
            None => line.ends_with(".ok();") && bare(&statement_to(&lines[..=i])),
        };
        if dropped {
            found.push(Finding {
                path: path.to_string(),
                line: i + 1,
                rule: RULE,
                message: "an editor action's error must reach the developer: pass it to \
                          the console (`crate::report::failed` in the editor), or say why it \
                          is best-effort with `// lint: allow(dropped-result) <why>`"
                    .to_string(),
            });
        }
    }
    found
}

/// The statement opening on `lines[0]`, from `rest`, that line's text after `let _ =`.
fn statement_from(lines: &[&str], rest: &str) -> String {
    let mut text = rest.to_string();
    let mut next = lines.iter().take(MAX_STATEMENT);
    let mut line = next.next().copied().unwrap_or(";");
    while !line.ends_with(';') {
        let Some(l) = next.next() else { break };
        line = l;
        text.push(' ');
        text.push_str(line);
    }
    text
}

/// The statement ending on the last of `lines`, its earlier lines back to the one
/// after a `;`, `{` or `}`.
fn statement_to(lines: &[&str]) -> String {
    let (last, before) = lines.split_last().map_or(("", &[][..]), |(l, b)| (*l, b));
    let ends = |l: &&&str| l.is_empty() || l.ends_with([';', '{', '}']);
    let mut start: Vec<&str> = before
        .iter()
        .rev()
        .take(MAX_STATEMENT)
        .take_while(|l| !ends(l))
        .copied()
        .collect();
    start.reverse();
    start.push(last);
    start.join(" ")
}

/// Whether an expression calls something, `f(…)` or `x.m(…)`, rather than naming
/// values (`let _ = (a, b);` silences unused bindings).
fn calls(expr: &str) -> bool {
    let mut prev = ' ';
    expr.chars().any(|c| {
        let call = c == '(' && (is_ident(prev) || matches!(prev, '>' | ')' | '!'));
        prev = c;
        call
    })
}

/// Whether a statement ending in `.ok();` drops its value: no binding, assignment
/// or `return` takes it.
fn bare(statement: &str) -> bool {
    let takes = ["let ", "return", "break"];
    !takes.iter().any(|t| statement.starts_with(t)) && !statement.contains(" = ")
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "dropped_result_tests.rs"]
mod tests;
