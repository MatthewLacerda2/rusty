//! The crates a manifest depends on: the set the scan looks for, and the set every
//! one of which the table must account for.

/// The crate names (`-` read as `_`) under `[dependencies]` and every
/// `[target.….dependencies]`; dev- and build-dependencies are not the modules'.
/// The workspace's own crates (`rusty-<module>`) are modules, not external
/// crates: the module half of the table holds them.
pub fn dependencies(manifest: &str) -> Vec<String> {
    let mut out = keys(manifest);
    out.retain(|k| !k.starts_with("rusty_"));
    out
}

/// Every crate name (`-` read as `_`) under `[dependencies]` and every
/// `[target.….dependencies]`, the workspace's own included, sorted.
pub fn keys(manifest: &str) -> Vec<String> {
    table_keys(manifest, "dependencies")
}

/// Every crate name (`-` read as `_`) under `[dev-dependencies]` and every
/// `[target.….dev-dependencies]`, the workspace's own included, sorted.
pub fn dev_keys(manifest: &str) -> Vec<String> {
    table_keys(manifest, "dev-dependencies")
}

/// The keys of the table `[<table>]` and every `[target.….<table>]`, and the name
/// in each one-crate header, `[<table>.<name>]` (`[target.….<table>.<name>]`).
fn table_keys(manifest: &str, table: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_deps = false;
    for raw in manifest.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_deps = false;
            match header_crate(header, table) {
                Some(None) => in_deps = true,
                Some(Some(name)) => out.push(name.replace('-', "_")),
                None => {}
            }
            continue;
        }
        let Some((key, _)) = line.split_once('=').filter(|_| in_deps) else {
            continue;
        };
        // `serde.workspace = true` names `serde`.
        let key = key.trim().split('.').next().unwrap_or("");
        if key.starts_with(|c: char| c.is_ascii_alphabetic()) {
            out.push(key.replace('-', "_"));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// For a table header's inside: `Some(None)` when it is `<table>` (or a target's),
/// `Some(Some(name))` when it is one crate's `<table>.<name>`, `None` otherwise.
fn header_crate<'a>(header: &'a str, table: &str) -> Option<Option<&'a str>> {
    let rest = match header.strip_prefix("target.") {
        // A target's cfg is quoted and may hold dots: the table follows its closing
        // quote. A bare triple (`aarch64-apple-darwin`) holds none: its first dot.
        Some(t) if t.starts_with(['\'', '"']) => t
            .split_once("'.")
            .or_else(|| t.split_once("\"."))
            .map(|(_, r)| r)?,
        Some(t) => t.split_once('.').map(|(_, r)| r)?,
        None => header,
    };
    let rest = rest.strip_prefix(table)?;
    if rest.is_empty() {
        return Some(None);
    }
    let name = rest.strip_prefix('.')?.trim_matches(['"', '\'']);
    // A name that is no crate's is no table of crates either: its keys are not read.
    name.starts_with(|c: char| c.is_ascii_alphabetic())
        .then_some(Some(name))
}

#[cfg(test)]
#[path = "cargo_tests.rs"]
mod tests;
