//! `converter-log`: a converter reports, never logs or discards (#1172).
//!
//! A converter that logs a failure and carries on returns `Ok`, and the import
//! cache stamps a file missing a part as current (#1168); one that logs a skip is
//! heard once, then never again from the cache. So a converter returns what it
//! skipped in `Converted` and fails on anything else, and `convert::refresh`
//! alone warns. Under `src/asset/convert/`, every module but [`NOT_CONVERTERS`]
//! is a converter's, a folder or a single file, and a code line there that names
//! `log::` or a bare logging macro is a finding. Its root (`<name>/mod.rs` or
//! `<name>.rs`) must also carry [`DENY`], however rustfmt wraps it: clippy, not
//! this scan, then refuses `let _ =` on a `Result`, a discarded `.ok()` and a
//! print there. A `drop(result)` or `_ = expr;` gets past both. A real need says
//! why with `// lint: allow(converter-log) <why>`.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::{strip_comment, Open};

pub const RULE: &str = "converter-log";
const ROOT: &str = "src/asset/convert/";
/// Modules under [`ROOT`] that are not converters: the cache itself, the table of
/// converters, the files a conversion depends on, the switch Play turns off, its
/// lock and its stamp record, which report, the swap, whether a cache is current,
/// what resolves a texture
/// reference through the cache, and the cache's tests. A new module of the cache's
/// own goes here.
const NOT_CONVERTERS: &[&str] = &[
    "converter",
    "depends",
    "gate",
    "lock",
    "mod",
    "record",
    "stage",
    "stale",
    "tests",
    "texture_ref",
];
/// What a converter's root module carries: every lint it names, in one inner
/// `deny`, wrapped or not.
pub const DENY: &str = "#![deny(clippy::let_underscore_must_use, clippy::unused_result_ok, \
                        clippy::print_stdout, clippy::print_stderr)]";
/// What a converter must not call.
const LOGGING: &[&str] = &[
    "log::", "log!", "warn!", "error!", "info!", "debug!", "trace!",
];

/// Every logging line of a converter's source, and a converter root without
/// [`DENY`].
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    let Some(name) = converter(path) else {
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
    let root = path == format!("{ROOT}{name}/mod.rs") || path == format!("{ROOT}{name}.rs");
    if root && !denies(source) {
        found.push(finding(
            1,
            &format!("a converter's root carries `{DENY}` (a cache module: `NOT_CONVERTERS`)"),
        ));
    }
    found
}

/// The converter `path` belongs to, if any: its folder, or its file's stem.
fn converter(path: &str) -> Option<&str> {
    let rest = path.strip_prefix(ROOT)?;
    let name = match rest.split_once('/') {
        Some((folder, _)) => folder,
        None => rest.strip_suffix(".rs")?,
    };
    (!NOT_CONVERTERS.contains(&name)).then_some(name)
}

/// Does `code` call a logging macro, with no identifier character before it?
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

/// Does `source` carry an inner `#![deny(…)]` naming every lint [`DENY`] names?
/// Read across the lines rustfmt wraps it over, comments stripped.
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

/// The lints a whole `#![deny(…)]` names; `None` while it is not closed yet.
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

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "converter_log_tests.rs"]
mod tests;
