//! src/scripting/api_doc.rs — the scripting-API reference, assembled (#569, #931).
//!
//! The reference lives in `docs/api/`: `index.md` plus one file per Lua namespace, so
//! two branches documenting different namespaces never touch the same file. The drift
//! gates and the dev-only MCP resource want it whole, so this concatenates it: the
//! index first, then every namespace file in the order the index links them, then
//! any file the index forgot (sorted).
//!
//! It reads the checkout at run time rather than embedding the text at build time,
//! because an embedded doc is part of the crate's fingerprint: every doc edit would
//! rebuild the whole crate once per feature set (#931). Its callers are tests and
//! `dev` tools, which always run from the engine checkout.

use std::fs;
use std::path::{Path, PathBuf};

/// `docs/api/` in the checkout this binary was built from.
pub fn api_doc_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/api")
}

/// The scripting-API reference: `docs/api/` assembled into one document.
pub fn api_reference() -> Result<String, String> {
    assemble(&api_doc_dir())
}

/// Assemble the reference in `dir`.
pub fn assemble(dir: &Path) -> Result<String, String> {
    let read = |name: &str| {
        fs::read_to_string(dir.join(name))
            .map_err(|e| format!("Failed to read {}: {e}", dir.join(name).display()))
    };
    let index = read("index.md")?;
    let mut order: Vec<String> = index
        .split("](")
        .skip(1)
        .filter_map(|rest| rest.split_once(')'))
        .map(|(target, _)| target.to_string())
        .filter(|t| t.ends_with(".md") && !t.contains('/') && dir.join(t).is_file())
        .collect();
    let mut rest: Vec<String> = fs::read_dir(dir)
        .map_err(|e| format!("Failed to list {}: {e}", dir.display()))?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".md") && n != "index.md" && !order.contains(n))
        .collect();
    rest.sort();
    order.dedup();
    order.extend(rest);

    let mut doc = index;
    for name in &order {
        doc.push_str("\n---\n\n");
        doc.push_str(&read(name)?);
    }
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `assemble` keeps the index's links in their order, drops a link that is not a
    /// sibling `.md` file, then appends the unlinked `.md` files, each exactly once.
    #[test]
    fn assemble_keeps_linked_then_unlinked_namespace_files_once() {
        let dir = crate::test_temp::dir().join("api_doc");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        let index = "# Index\n[z](z.md) [a](a.md) [b](sub/b.md) [n](notes.txt) [m](missing.md)\n";
        for (name, text) in [
            ("index.md", index),
            ("z.md", "Z\n"),
            ("a.md", "A\n"),
            ("extra.md", "E\n"),
            ("notes.txt", "N\n"),
            ("sub/b.md", "B\n"),
        ] {
            fs::write(dir.join(name), text).unwrap();
        }
        let doc = assemble(&dir);
        let _ = fs::remove_dir_all(&dir);
        let part = |text: &str| format!("\n---\n\n{text}");
        let want = [index.to_string(), part("Z\n"), part("A\n"), part("E\n")].concat();
        assert_eq!(doc, Ok(want));
    }
}
