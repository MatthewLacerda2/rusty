//! why with `// lint: allow(converter-log) <why>`.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::{strip_comment, Open};

pub const RULE: &str = "converter-log";
const ROOT: &str = "src/asset/convert/";
const NOT_CONVERTERS: &[&str] = &[
    "converter",
    "depends",
    "fingerprint",
    "gate",
    "lock",
    "mod",
    "record",
    "stage",
    "stale",
    "tests",
    "texture_ref",
];
pub const DENY: &str = "#![deny(clippy::let_underscore_must_use, clippy::unused_result_ok, \
                        clippy::print_stdout, clippy::print_stderr)]";
const LOGGING: &[&str] = &[
    "log::", "log!", "warn!", "error!", "info!", "debug!", "trace!",
];

pub fn check(path: &str, source: &str) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    let Some(name) = converter(&at) else {
        return Vec::new();
    };
    let finding = |line, message: &str| Finding {
        path: path.to_string(),
        line,
        rule: RULE,
        message: message.to_string(),
    };
    let mut open = Open::None;
    let mut found = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let code = strip_comment(line, &mut open);
        if logs(&code) {
            found.push(finding(
                i + 1,
                "a converter never logs: return the skip in `Converted`, or fail",
            ));
        }
    }
    let root = at == format!("{ROOT}{name}/mod.rs") || at == format!("{ROOT}{name}.rs");
    if root && !denies(source) {
        found.push(finding(
            1,
            &format!("a converter's root carries `{DENY}` (a cache module: `NOT_CONVERTERS`)"),
        ));
    }
    found
}

fn converter(path: &str) -> Option<&str> {
    let rest = path.strip_prefix(ROOT)?;
    let name = match rest.split_once('/') {
        Some((folder, _)) => folder,
        None => rest.strip_suffix(".rs")?,
    };
    (!NOT_CONVERTERS.contains(&name)).then_some(name)
}

fn logs(code: &str) -> bool {
    LOGGING.iter().any(|name| {
        code.match_indices(name).any(|(at, _)| {
            !code[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
        })
    })
}

fn denies(source: &str) -> bool {
    let wanted = lints(DENY).unwrap_or_default();
    let mut open = Open::None;
    let mut attr: Option<String> = None;
    for line in source.lines() {
        let code = strip_comment(line, &mut open);
        let code = code.trim();
        let text = match attr.take() {
            Some(text) => text + code,
            None if code.starts_with("#![deny(") => code.to_string(),
            None => continue,
        };
        match lints(&text) {
            Some(named) if wanted.iter().all(|l| named.contains(l)) => return true,
            Some(_) => {}
            None => attr = Some(text),
        }
    }
    false
}

fn lints(attr: &str) -> Option<Vec<&str>> {
    let inner = attr.strip_prefix("#![deny(")?.strip_suffix(")]")?;
    Some(
        inner
            .split(',')
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect(),
    )
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "converter_log_tests.rs"]
mod tests;
