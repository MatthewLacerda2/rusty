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
    let mut out = Vec::new();
    let mut in_deps = false;
    for raw in manifest.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            in_deps = line == "[dependencies]"
                || (line.starts_with("[target.") && line.ends_with(".dependencies]"));
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
