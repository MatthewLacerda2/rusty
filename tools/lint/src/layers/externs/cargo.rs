pub fn dependencies(manifest: &str) -> Vec<String> {
    let mut out = keys(manifest);
    out.retain(|k| !k.starts_with("rusty_"));
    out
}

pub fn keys(manifest: &str) -> Vec<String> {
    table_keys(manifest, "dependencies")
}

pub fn dev_keys(manifest: &str) -> Vec<String> {
    table_keys(manifest, "dev-dependencies")
}

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
        let key = key.trim().split('.').next().unwrap_or("");
        if key.starts_with(|c: char| c.is_ascii_alphabetic()) {
            out.push(key.replace('-', "_"));
        }
    }
    out.sort();
    out.dedup();
    out
}

fn header_crate<'a>(header: &'a str, table: &str) -> Option<Option<&'a str>> {
    let rest = match header.strip_prefix("target.") {
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
    name.starts_with(|c: char| c.is_ascii_alphabetic())
        .then_some(Some(name))
}

#[cfg(test)]
#[path = "cargo_tests.rs"]
mod tests;
