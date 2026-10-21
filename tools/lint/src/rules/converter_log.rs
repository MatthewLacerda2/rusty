//! `converter-log`: a converter reports, never logs (#1172).
//!
//! A converter that logs a failure and carries on returns `Ok`, and the import
//! cache stamps a file missing a part as current (#1168); one that logs a skip is
//! heard once, then never again from the cache. So a converter returns what it
//! skipped in `Converted` and fails on anything else, and `convert::refresh`
//! alone warns. Under `src/asset/convert/<folder>/`, every folder but `stage` (the
//! swap) and `tests` is a converter's, and a code line there that names `log::`,
//! a bare logging macro (`warn!`, `error!`, `info!`, `debug!`, `trace!`) or a
//! print (`println!`, `eprintln!`) is a finding. Its `mod.rs` must also carry
//! [`DENY`], so a `let _ =` cannot drop a `Result` there. A real need says why
//! with `// lint: allow(converter-log) <why>`.

use super::finding::Finding;
use super::scan::Scan;

pub const RULE: &str = "converter-log";
const ROOT: &str = "src/asset/convert/";
/// Folders under [`ROOT`] that are not converters: the swap, which may report,
/// and the cache's tests.
const NOT_CONVERTERS: &[&str] = &["stage", "tests"];
/// What a converter folder's `mod.rs` carries.
pub const DENY: &str = "#![deny(clippy::let_underscore_must_use)]";
/// What a converter must not call.
const LOGGING: &[&str] = &[
    "log::",
    "warn!",
    "error!",
    "info!",
    "debug!",
    "trace!",
    "println!",
    "eprintln!",
];

/// Every logging line of a converter's source, and a converter root without [`DENY`].
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    let Some(folder) = converter(path) else {
        return Vec::new();
    };
    let finding = |line, message: &str| Finding {
        path: path.to_string(),
        line,
        rule: RULE,
        message: message.to_string(),
    };
    let mut found: Vec<Finding> = source
        .lines()
        .enumerate()
        .filter(|(_, l)| logs(l.split("//").next().unwrap_or_default()))
        .map(|(i, _)| {
            finding(
                i + 1,
                "a converter never logs: return the skip in `Converted`, or fail",
            )
        })
        .collect();
    let root = path == format!("{ROOT}{folder}/mod.rs");
    if root && !source.lines().any(|l| l.trim() == DENY) {
        found.push(finding(
            1,
            &format!("a converter folder's mod.rs carries `{DENY}`"),
        ));
    }
    found
}

/// The converter folder `path` lies in, if any.
fn converter(path: &str) -> Option<&str> {
    let (folder, _) = path.strip_prefix(ROOT)?.split_once('/')?;
    (!NOT_CONVERTERS.contains(&folder)).then_some(folder)
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

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "converter_log_tests.rs"]
mod tests;
