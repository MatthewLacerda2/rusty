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
        // A target's cfg is quoted and may hold dots: the table follows its closing quote.
        Some(t) => t
            .split_once("'.")
            .or_else(|| t.split_once("\"."))
            .map(|(_, r)| r)?,
        None => header,
    };
    let rest = rest.strip_prefix(table)?;
    if rest.is_empty() {
        return Some(None);
    }
    let name = rest.strip_prefix('.')?.trim_matches(['"', '\'']);
    Some(Some(name).filter(|n| n.starts_with(|c: char| c.is_ascii_alphabetic())))
}

#[cfg(test)]
mod tests {
    use super::{dev_keys, keys};

    #[test]
    fn a_one_crate_header_names_its_crate() {
        let text = "[dependencies.serde]\nversion = \"1\"\n\n[dev-dependencies.rusty-render]\n\
            workspace = true\n[target.'cfg(target_os = \"macos\")'.dev-dependencies.objc2]\nworkspace = true\n";
        assert_eq!(keys(text), ["serde"]);
        assert_eq!(dev_keys(text), ["objc2", "rusty_render"]);
    }

    #[test]
    fn a_table_with_a_lookalike_name_is_not_read() {
        let text = "[dependencies-extra]\nfoo = 1\n[package.metadata.dependencies]\nbar = 1\n";
        assert!(keys(text).is_empty());
    }
}
