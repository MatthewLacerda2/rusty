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

/// The keys of the table `[<table>]` and every `[target.….<table>]`.
fn table_keys(manifest: &str, table: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_deps = false;
    let plain = format!("[{table}]");
    let target = format!(".{table}]");
    for raw in manifest.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            in_deps =
                line == plain || (line.starts_with("[target.") && line.ends_with(target.as_str()));
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
