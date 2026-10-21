//! `converter-log`: a converter reports, never logs or discards (#1172).
//!
//! A converter that logs a failure and carries on returns `Ok`, and the import
//! cache stamps a file missing a part as current (#1168); one that logs a skip is
//! heard once, then never again from the cache. So a converter returns what it
//! skipped in `Converted` and fails on anything else, and `convert::refresh`
//! alone warns. Under `src/asset/convert/`, every module but [`NOT_CONVERTERS`]
//! is a converter's, a folder or a single file, and a code line there that names
//! `log::`, a bare logging macro or a print is a finding, as is one that throws a
//! result away with `_ = …` or `.ok();`. Its root (`<name>/mod.rs` or `<name>.rs`)
//! must also carry [`DENY`], so `let _ =` cannot drop a `Result` there. A
//! `drop(result)` gets past both. A real need says why with
//! `// lint: allow(converter-log) <why>`.

use super::finding::Finding;
use super::scan::Scan;
use crate::layers::{strip_comment, Open};

pub const RULE: &str = "converter-log";
const ROOT: &str = "src/asset/convert/";
/// Modules under [`ROOT`] that are not converters: the cache itself, its lock and
/// its stamp record, which report, the swap, and the cache's tests. A new module
/// of the cache's own goes here.
const NOT_CONVERTERS: &[&str] = &["lock", "mod", "record", "stage", "tests"];
/// What a converter's root module carries.
pub const DENY: &str = "#![deny(clippy::let_underscore_must_use)]";
/// What a converter must not call.
const LOGGING: &[&str] = &[
    "log::",
    "log!",
    "warn!",
    "error!",
    "info!",
    "debug!",
    "trace!",
    "print!",
    "println!",
    "eprint!",
    "eprintln!",
];

/// Every logging or discarding line of a converter's source, and a converter
/// root without [`DENY`].
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
        if logs(code) {
            found.push(finding(
                i + 1,
                "a converter never logs: return the skip in `Converted`, or fail",
            ));
        } else if discards(code) {
            found.push(finding(
                i + 1,
                "a converter never discards a result: return it with `?`, or a skip",
            ));
        }
    }
    let root = path == format!("{ROOT}{name}/mod.rs") || path == format!("{ROOT}{name}.rs");
    if root && !source.lines().any(|l| l.trim() == DENY) {
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

/// Does `code` throw a result away: `_ = expr;`, or a statement ending `.ok();`
/// whose value nothing binds?
fn discards(code: &str) -> bool {
    let code = code.trim();
    let binds = code.starts_with("let ") || code.contains(" = ");
    let assigns = code
        .strip_prefix("_ =")
        .is_some_and(|r| !r.starts_with('>'));
    assigns || code.ends_with(".ok();") && !binds
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "converter_log_tests.rs"]
mod tests;
